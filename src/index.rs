use rocket::get;
use rocket::post;
use rocket::response::content::RawHtml;
use rocket::response::status;
use crate::{get_all_display_configs, load_monitor_config};

#[get("/")]
pub async fn index() -> RawHtml<String> {
    let configs = get_all_display_configs().await.unwrap_or_default();

    let mut html = String::from(r#"
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
            }
            .config-name {
                font-size: 1.2em;
                font-weight: bold;
                color: #333;
            }
            .config-id {
                color: #666;
                font-size: 0.9em;
            }
            .apply-button {
                background: #4CAF50;
                color: white;
                border: none;
                padding: 10px 20px;
                border-radius: 4px;
                cursor: pointer;
                font-size: 1em;
                transition: background 0.3s;
            }
            .apply-button:hover {
                background: #45a049;
            }
            .apply-button:active {
                background: #3d8b40;
            }
        </style>
    </head>
    <body>
        <h1>Monitor Configurations</h1>
        <div class="config-grid">
    "#);

    for config in configs {
        html.push_str(&format!(
            r#"<div class="config-item">
                <span class="config-name">{}</span>
                <span class="config-id">ID: {}</span>
                <button class="apply-button" onclick="applyConfig('{}')">Apply Configuration</button>
            </div>"#,
            config.name, config.id, config.id
        ));
    }

    html.push_str(r#"
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
    "#);

    RawHtml(html)
}

#[post("/api/apply/<id>")]
pub async fn apply_config(id: &str) -> status::Accepted<String> {
    if let Ok(Some(config)) = load_monitor_config(&id).await {
        if let Ok(_) = config.display_config.set() {
            return status::Accepted(format!("Configuration '{}' applied successfully", config.name));
        }
    }
    status::Accepted("Failed to apply configuration".to_string())
}
