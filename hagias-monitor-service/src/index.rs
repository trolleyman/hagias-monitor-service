use anyhow::Result;
use rocket::http::Status;
use rocket::post;
use rocket::response::content::RawHtml;
use rocket::response::status;
use rocket::{State, get};

use crate::config::Config;
use crate::layouts::Layouts;

#[get("/")]
pub async fn index(
    config: &State<Config>,
) -> Result<RawHtml<String>, rocket::response::Debug<anyhow::Error>> {
    let layouts = Layouts::load(&config.layouts_path.relative()).await?;
    let mut html = String::from(
        r#"
    <!DOCTYPE html>
    <html>
    <head>
        <title>Monitor Configurations</title>
        <style>
            body {
                font-family: Arial, sans-serif;
                max-width: 1200px;
                margin: 0 auto;
                padding: 20px;
            }
            .config-grid {
                display: grid;
                grid-template-columns: repeat(2, 1fr);
                gap: 20px;
                padding: 0;
            }
            .config-item {
                background: #f5f5f5;
                padding: 20px;
                border-radius: 8px;
                display: flex;
                flex-direction: column;
                gap: 10px;
                position: relative;
                cursor: pointer;
                border: none;
                transition: all 0.3s;
            }
            .config-item:hover {
                background: #e0e0e0;
                transform: translateY(-2px);
                box-shadow: 0 4px 8px rgba(0,0,0,0.1);
            }
            .config-name {
                font-size: 1.2em;
                font-weight: bold;
                color: #333;
                margin-top: 20px;
            }
            .config-id {
                color: #666;
                font-size: 0.9em;
                position: absolute;
                top: 10px;
                right: 10px;
            }
        </style>
    </head>
    <body>
        <h1>Monitor Configurations</h1>
        <div class="config-grid">
    "#,
    );

    for layout in layouts {
        html.push_str(&format!(
            r#"<button class="config-item" onclick="applyConfig('{0}')">
                <span class="config-id">{0}</span>
                <span class="config-name">{1}</span>
            </button>"#,
            html_escape::encode_safe(&layout.id),
            html_escape::encode_safe(&layout.name)
        ));
    }

    html.push_str(
        r#"
        </div>
        <script>
            async function applyConfig(id) {
                try {
                    const response = await fetch('/api/apply/' + id, {
                        method: 'POST'
                    });
                    if (response.ok) {
                        alert('Configuration applied successfully!');
                    } else {
                        alert('Failed to apply configuration');
                    }
                } catch (error) {
                    alert('Error applying configuration: ' + error);
                }
            }
        </script>
    </body>
    </html>
    "#,
    );

    Ok(RawHtml(html))
}

#[post("/api/apply/<id>")]
pub async fn apply_config(id: String, config: &State<Config>) -> status::Custom<String> {
    match Layouts::load(&config.layouts_path.relative()).await {
        Ok(layouts) => match layouts.get_layout(&id) {
            Some(layout) => match layout.layout.apply() {
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
