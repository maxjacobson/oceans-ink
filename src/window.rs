mod imp {
    use adw::prelude::*;
    use adw::subclass::prelude::*;
    use gtk::gio;
    use gtk::glib;

    #[derive(gtk::CompositeTemplate, Default)]
    #[template(resource = "/net/hardscrabble/oceans-ink/ui/window.ui")]
    pub struct Window {
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub token_entry: TemplateChild<adw::PasswordEntryRow>,
        #[template_child]
        pub save_button: TemplateChild<gtk::Button>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Window {
        const NAME: &'static str = "OceansInkWindow";
        type Type = super::Window;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Window {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();

            self.save_button.connect_clicked(glib::clone!(
                #[weak]
                obj,
                move |_| obj.save_token_clicked()
            ));

            glib::spawn_future_local(glib::clone!(
                #[weak]
                obj,
                async move {
                    let has_token =
                        gio::spawn_blocking(|| crate::secret::get_token().is_some())
                            .await
                            .unwrap_or(false);
                    obj.show_ready_or_token_page(has_token);
                }
            ));
        }
    }

    impl WidgetImpl for Window {}
    impl WindowImpl for Window {}
    impl ApplicationWindowImpl for Window {}
    impl AdwApplicationWindowImpl for Window {}
}

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::gio;
use gtk::glib;

glib::wrapper! {
    pub struct Window(ObjectSubclass<imp::Window>)
        @extends adw::ApplicationWindow, gtk::ApplicationWindow, gtk::Window, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Native,
            gtk::Root, gtk::ShortcutManager, gio::ActionGroup, gio::ActionMap;
}

impl Window {
    pub fn new(app: &adw::Application) -> Self {
        glib::Object::builder().property("application", app).build()
    }

    fn save_token_clicked(&self) {
        let entry = &self.imp().token_entry;
        let token = entry.text().trim().to_string();
        if token.is_empty() {
            entry.add_css_class("error");
            return;
        }
        entry.remove_css_class("error");
        entry.set_sensitive(false);

        glib::spawn_future_local(glib::clone!(
            #[weak(rename_to = obj)]
            self,
            async move {
                let saved = gio::spawn_blocking(move || crate::secret::save_token(&token).is_ok())
                    .await
                    .unwrap_or(false);

                let entry = &obj.imp().token_entry;

                if saved {
                    obj.show_ready_or_token_page(true);
                } else {
                    entry.set_sensitive(true);
                    entry.add_css_class("error");
                }
            }
        ));
    }

    fn show_ready_or_token_page(&self, has_token: bool) {
        let page = if has_token { "ready" } else { "token" };
        self.imp().stack.set_visible_child_name(page);
    }
}
