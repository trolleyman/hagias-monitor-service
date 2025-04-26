use anyhow::Result;
use rocket::http::Status;
use rocket::post;
use rocket::response::status;
use rocket::{State, get};
use rocket_dyn_templates::{Template, context};

use crate::config::Config;
use crate::layouts::Layouts;

#[get("/")]
pub async fn index(
    config: &State<Config>,
) -> Result<Template, rocket::response::Debug<anyhow::Error>> {
    let layouts = Layouts::load(&config.layouts_path.relative()).await?;
    Ok(Template::render("index", context! {
        layouts: layouts.iter().collect::<Vec<_>>()
    }))
}

#[post("/api/apply/<id>")]
pub async fn apply_config(id: &str, config: &State<Config>) -> status::Custom<String> {
    match Layouts::load(&config.layouts_path.relative()).await {
        Ok(layouts) => match layouts.get_layout(&id) {
            Some(layout) => match layout.layout.apply(true) { // TODO: Have an /api/confirm that saves the layout to the database (or defaultable arg here)
                Ok(_) => status::Custom(
                    Status::Accepted,
                    format!(
                        "Configuration {} \"{}\" applied successfully",
                        layout.id, layout.name
                    ),
                ),
                Err(e) => status::Custom(
                    Status::InternalServerError,
                    format!(
                        "Failed to apply layout {} \"{}\": {:?}",
                        layout.id, layout.name, e
                    ),
                ),
            },
            None => status::Custom(Status::NotFound, format!("Layout {} not found", id)),
        },
        Err(e) => status::Custom(
            Status::InternalServerError,
            format!("Failed to load layouts: {:?}", e),
        ),
    }
}
