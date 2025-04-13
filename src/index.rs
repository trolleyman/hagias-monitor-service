use rocket::get;
use rocket::post;
use rocket::response::content::RawHtml;
use rocket::response::status;
use crate::config::get_all_display_configs;
use crate::config::load_monitor_config;

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
    "#);

    for config in configs {
        html.push_str(&format!(
            r#"<button class="config-item" onclick="applyConfig('{}')">
                <span class="config-id">{}</span>
                <span class="config-name">{}</span>
            </button>"#,
            config.id, config.id, config.name
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
pub async fn apply_config(id: String) -> status::Accepted<String> {
    if let Ok(Some(config)) = load_monitor_config(&id).await {
        if let Ok(_) = config.display_config.set() {
            return status::Accepted(format!("Configuration '{}' applied successfully", config.name));
        }
    }
    status::Accepted("Failed to apply configuration".to_string())
}
