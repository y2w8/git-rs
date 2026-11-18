use crate::{markdown, prelude::*, routes::PageQuery};

#[derive(Template)]
#[template(path = "repository/about.html")]
struct AboutPage {
    repo: Repository,
    current_ref: Ref,
    branch_names: Vec<String>,
    readme_html: Option<String>,
    query: web::Query<PageQuery>,
}

#[get("/{repo}/about/")]
pub async fn page(
    path: web::Path<String>,
    query: web::Query<PageQuery>,
    data: web::Data<App>,
) -> Result<HttpResponse, Error> {
    let repo = data.get_repo(&path)?;
    let current_ref = Ref::resolve(&repo, &query)?;
    let branch_names = repo.branch_names()?;
    let readme_html = repo
        .readme(&current_ref)
        .map(|raw| markdown::render(&raw, &repo.name, &current_ref.name));

    render_html(&AboutPage {
        repo,
        current_ref,
        branch_names,
        readme_html,
        query,
    })
}
