mod instapaper;
mod secret;
mod window;

use adw::prelude::*;
use gtk::gio;
use gtk::glib;

const APP_ID: &str = "net.hardscrabble.oceans-ink";

fn register_resources() {
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();

    if let Some(path) = std::env::var_os("OCEANS_INK_GRESOURCE") {
        candidates.push(std::path::PathBuf::from(path));
    }

    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(std::path::Path::to_path_buf))
    {
        candidates.push(dir.join("../../share/oceans-ink/oceans-ink.gresource"));
        candidates.push(dir.join("../share/oceans-ink/oceans-ink.gresource"));
        candidates.push(dir.join("share/oceans-ink/oceans-ink.gresource"));
    }

    candidates.push("/app/share/oceans-ink/oceans-ink.gresource".into());
    candidates.push("/usr/share/oceans-ink/oceans-ink.gresource".into());

    for path in &candidates {
        if let Ok(resource) = gio::Resource::load(path) {
            gio::resources_register(&resource);
            return;
        }
    }

    panic!(
        "Could not load UI resources. Tried: {:?}. \
         When building with meson, set OCEANS_INK_GRESOURCE to the built oceans-ink.gresource.",
        candidates
    );
}

fn main() -> glib::ExitCode {
    let app = adw::Application::builder().application_id(APP_ID).build();

    app.connect_startup(|_| {
        register_resources();

        let css = gtk::CssProvider::new();
        css.load_from_string(
            "popover.menu button { font-weight: normal; } \
             .oi-heart { opacity: 0.4; } \
             .oi-heart.oi-liked { color: #e01b24; opacity: 1; }",
        );
        if let Some(display) = gtk::gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &css,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    });

    let quit = gtk::gio::SimpleAction::new("quit", None);
    let app_weak = app.downgrade();
    quit.connect_activate(move |_, _| {
        if let Some(app) = app_weak.upgrade() {
            app.quit();
        }
    });
    app.add_action(&quit);
    app.set_accels_for_action("app.quit", &["<Ctrl>q"]);

    app.connect_activate(|app| {
        if let Some(existing) = app.active_window() {
            existing.present();
            return;
        }
        window::Window::new(app).present();
    });

    app.run()
}
