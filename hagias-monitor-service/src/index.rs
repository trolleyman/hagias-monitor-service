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
            :root {
                --bg-primary: #1a1a1a;
                --bg-secondary: #2d2d2d;
                --text-primary: #ffffff;
                --text-secondary: #b3b3b3;
                --accent-color: #4a90e2;
                --hover-color: #3a7bc8;
            }

            body {
                font-family: 'Segoe UI', Arial, sans-serif;
                max-width: 1200px;
                margin: 0 auto;
                padding: 20px;
                background-color: var(--bg-primary);
                color: var(--text-primary);
            }

            h1 {
                color: var(--text-primary);
                margin-bottom: 30px;
                font-size: 2.5em;
                font-weight: 600;
                text-align: center;
            }

            .config-grid {
                display: grid;
                grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
                gap: 20px;
                padding: 0;
            }

            .config-item {
                background: var(--bg-secondary);
                padding: 25px;
                border-radius: 12px;
                display: flex;
                flex-direction: column;
                gap: 15px;
                position: relative;
                cursor: pointer;
                border: 1px solid rgba(255, 255, 255, 0.1);
                transition: all 0.3s ease;
            }

            .config-item:hover {
                background: var(--accent-color);
                transform: translateY(-5px);
                box-shadow: 0 8px 16px rgba(0, 0, 0, 0.2);
            }

            .config-name {
                font-size: 1.3em;
                font-weight: 600;
                color: var(--text-primary);
                margin-top: 20px;
            }

            .config-id {
                color: var(--text-secondary);
                font-size: 0.9em;
                position: absolute;
                top: 15px;
                right: 15px;
                background: rgba(0, 0, 0, 0.2);
                padding: 4px 8px;
                border-radius: 4px;
            }

            .config-emoji {
                position: absolute;
                top: 15px;
                left: 15px;
                font-size: 1.5em;
                background: rgba(0, 0, 0, 0.2);
                padding: 4px 8px;
                border-radius: 4px;
            }

            @media (max-width: 768px) {
                .config-grid {
                    grid-template-columns: 1fr;
                }
            }
        </style>
    </head>
    <body>
        <h1>Monitor Configurations</h1>
        <div class="config-grid">
    "#,
    );

    for layout in layouts.iter().filter(|l| !l.hidden) {
        html.push_str(&format!(
            r#"<button class="config-item" onclick="applyConfig('{0}')">
                <span class="config-emoji">{2}</span>
                <span class="config-id">{0}</span>
                <span class="config-name">{1}</span>
            </button>"#,
            html_escape::encode_safe(&layout.id),
            html_escape::encode_safe(&layout.name),
            layout
                .emoji
                .as_ref()
                .map(|s| html_escape::encode_safe(s))
                .unwrap_or_default()
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
pub async fn apply_config(id: &str, config: &State<Config>) -> status::Custom<String> {
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
