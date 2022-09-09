use crate::{
  newtypes::{CommunityId, LanguageId, LocalUserId, SiteId},
  source::{actor_language::*, community::Community, language::Language, site::Site},
};
use diesel::{
  delete, dsl::*, insert_into, result::Error, select, ExpressionMethods, PgConnection, QueryDsl,
  RunQueryDsl,
};
use lemmy_utils::error::LemmyError;

impl LocalUserLanguage {
  pub fn read(
    conn: &mut PgConnection,
    for_local_user_id: LocalUserId,
  ) -> Result<Vec<LanguageId>, Error> {
    use crate::schema::local_user_language::dsl::*;

    local_user_language
      .filter(local_user_id.eq(for_local_user_id))
      .select(language_id)
      .get_results(conn)
  }

  /// Update the user's languages.
  ///
  /// If no language_id vector is given, it will show all languages
  pub fn update(
    conn: &mut PgConnection,
    language_ids: Vec<LanguageId>,
    for_local_user_id: LocalUserId,
  ) -> Result<(), Error> {
    conn.build_transaction().read_write().run(|conn| {
      use crate::schema::local_user_language::dsl::*;
      // Clear the current user languages
      delete(local_user_language.filter(local_user_id.eq(for_local_user_id))).execute(conn)?;

      let lang_ids = update_languages(conn, language_ids)?;
      for l in lang_ids {
        let form = LocalUserLanguageForm {
          local_user_id: for_local_user_id,
          language_id: l,
        };
        insert_into(local_user_language)
          .values(form)
          .get_result::<Self>(conn)?;
      }
      Ok(())
    })
  }
}

impl SiteLanguage {
  pub fn read_local(conn: &mut PgConnection) -> Result<Vec<LanguageId>, Error> {
    conn.build_transaction().read_write().run(|conn| {
      let local_site = Site::read_local(conn)?;
      SiteLanguage::read(conn, local_site.id)
    })
  }

  pub fn read(conn: &mut PgConnection, for_site_id: SiteId) -> Result<Vec<LanguageId>, Error> {
    use crate::schema::site_language::dsl::*;
    site_language
      .filter(site_id.eq(for_site_id))
      .select(language_id)
      .load(conn)
  }

  pub fn update(
    conn: &mut PgConnection,
    language_ids: Vec<LanguageId>,
    for_site_id: SiteId,
  ) -> Result<(), Error> {
    conn.build_transaction().read_write().run(|conn| {
      use crate::schema::site_language::dsl::*;
      // Clear the current languages
      delete(site_language.filter(site_id.eq(for_site_id))).execute(conn)?;

      let lang_ids = update_languages(conn, language_ids)?;
      for l in lang_ids.clone() {
        let form = SiteLanguageForm {
          site_id: for_site_id,
          language_id: l,
        };
        insert_into(site_language)
          .values(form)
          .get_result::<Self>(conn)?;
      }

      CommunityLanguage::limit_languages(conn, lang_ids)?;

      Ok(())
    })
  }
}

impl CommunityLanguage {
  /// Returns true if the given language is one of configured languages for given community
  pub fn is_allowed_community_language(
    conn: &mut PgConnection,
    for_language_id: LanguageId,
    for_community_id: CommunityId,
  ) -> Result<(), LemmyError> {
    use crate::schema::community_language::dsl::*;
    let is_allowed = select(exists(
      community_language
        .filter(language_id.eq(for_language_id))
        .filter(community_id.eq(for_community_id)),
    ))
    .get_result(conn)?;

    if is_allowed {
      Ok(())
    } else {
      Err(LemmyError::from_message("language_not_allowed"))
    }
  }

  /// When site languages are updated, delete all languages of local communities which are not
  /// also part of site languages. This is because post/comment language is only checked against
  /// community language, and it shouldnt be possible to post content in languages which are not
  /// allowed by local site.
  fn limit_languages(
    conn: &mut PgConnection,
    site_language_ids: Vec<LanguageId>,
  ) -> Result<(), Error> {
    // this could be implemented using join + delete, but its not supported in diesel yet
    // https://github.com/diesel-rs/diesel/issues/1478

    use crate::schema::{community::dsl::*, community_language::dsl::*};
    let local_communities: Vec<Community> = community.filter(local.eq(true)).load(conn)?;

    for c in local_communities {
      delete(
        community_language
          .filter(community_id.eq(c.id))
          .filter(language_id.ne_all(&site_language_ids)),
      )
      .execute(conn)?;
    }
    Ok(())
  }

  pub fn read(
    conn: &mut PgConnection,
    for_community_id: CommunityId,
  ) -> Result<Vec<LanguageId>, Error> {
    use crate::schema::community_language::dsl::*;
    community_language
      .filter(community_id.eq(for_community_id))
      .select(language_id)
      .load(conn)
  }

  pub fn update(
    conn: &mut PgConnection,
    language_ids: Vec<LanguageId>,
    for_community_id: CommunityId,
  ) -> Result<(), Error> {
    conn.build_transaction().read_write().run(|conn| {
      use crate::schema::community_language::dsl::*;
      // Clear the current languages
      delete(community_language.filter(community_id.eq(for_community_id))).execute(conn)?;

      let lang_ids = update_languages(conn, language_ids)?;
      for l in lang_ids {
        let form = CommunityLanguageForm {
          community_id: for_community_id,
          language_id: l,
        };
        insert_into(community_language)
          .values(form)
          .get_result::<Self>(conn)?;
      }
      Ok(())
    })
  }
}

// If no language is given, set all languages
fn update_languages(
  conn: &mut PgConnection,
  language_ids: Vec<LanguageId>,
) -> Result<Vec<LanguageId>, Error> {
  if language_ids.is_empty() {
    Ok(
      Language::read_all(conn)?
        .into_iter()
        .map(|l| l.id)
        .collect(),
    )
  } else {
    Ok(language_ids)
  }
}
