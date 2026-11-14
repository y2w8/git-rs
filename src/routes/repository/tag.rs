use crate::{git::tag::TaggedObject, prelude::*, routes::PageQuery};

#[derive(Template)]
#[template(path = "repository/tag.html")]
struct TagPage<'repo> {
    repo: &'repo Repository,
    branch_names: Vec<String>,
    current_ref: Ref,
    tag: Tag,
    query: web::Query<PageQuery>,
}

#[get("/{repo}/tag/")]
pub async fn page(
    path: web::Path<String>,
    query: web::Query<PageQuery>,
    data: web::Data<App>,
) -> Result<HttpResponse, Error> {
    let repo = data.get_repo(&path)?;
    let gix_repo = repo.gix();

    let current_ref = Ref::resolve(&repo, &query)?;
    let branch_names = repo.branch_names()?;
    let tag = if let Some(ref head) = query.head {
        repo.find_tag(head)?
    } else {
        let reference = gix_repo.find_reference(&format!("heads/{}", current_ref.name))?;
        Tag::from_gix(reference)?
    };

    render_html(&TagPage {
        repo: &repo,
        branch_names,
        current_ref,
        tag,
        query,
    })
}
