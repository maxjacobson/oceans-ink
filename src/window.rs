mod imp {
    use std::cell::{Cell, RefCell};
    use std::sync::{Arc, OnceLock};

    use adw::prelude::*;
    use adw::subclass::prelude::*;
    use gtk::glib;

    use crate::instapaper::{Client, Section};

    #[derive(gtk::CompositeTemplate)]
    #[template(resource = "/net/hardscrabble/oceans-ink/ui/window.ui")]
    pub struct Window {
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub token_entry: TemplateChild<adw::PasswordEntryRow>,
        #[template_child]
        pub save_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub sections_list: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub lists_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub content_title: TemplateChild<adw::WindowTitle>,
        pub(crate) sections: RefCell<Vec<super::SectionView>>,
        pub current_section: Cell<usize>,
        pub pending: Cell<Option<(usize, usize)>>,
        pub client: Arc<OnceLock<Arc<Client>>>,
    }

    impl Default for Window {
        fn default() -> Self {
            Self {
                stack: TemplateChild::default(),
                token_entry: TemplateChild::default(),
                save_button: TemplateChild::default(),
                sections_list: TemplateChild::default(),
                lists_stack: TemplateChild::default(),
                content_title: TemplateChild::default(),
                sections: RefCell::new(Vec::new()),
                current_section: Cell::new(0),
                pending: Cell::new(None),
                client: Arc::new(OnceLock::new()),
            }
        }
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

            for (index, section) in [Section::Home, Section::Liked, Section::Archive]
                .into_iter()
                .enumerate()
            {
                let view = super::SectionView::new(index, obj.downgrade());
                self.lists_stack
                    .add_named(view.stack(), Some(section.query_value()));
                self.sections.borrow_mut().push(view);
            }

            self.save_button.connect_clicked(glib::clone!(
                #[weak]
                obj,
                move |_| obj.save_token_clicked()
            ));

            self.sections_list.connect_row_activated(glib::clone!(
                #[weak]
                obj,
                move |_, row| obj.select_section(row.index() as usize)
            ));

            obj.setup_actions();
            obj.setup_key_navigation();

            glib::spawn_future_local(glib::clone!(
                #[weak]
                obj,
                async move {
                    let token = gio_spawn_blocking(crate::secret::get_token)
                        .await
                        .unwrap_or(None);
                    match token {
                        Some(token) => obj.init_with_token(token),
                        None => obj.show_token_page(),
                    }
                }
            ));
        }
    }

    fn gio_spawn_blocking<T, F>(f: F) -> gtk::gio::JoinHandle<T>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        gtk::gio::spawn_blocking(f)
    }

    impl WidgetImpl for Window {}
    impl WindowImpl for Window {}
    impl ApplicationWindowImpl for Window {}
    impl AdwApplicationWindowImpl for Window {}
}

use std::sync::Arc;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::glib;

use crate::instapaper::{Bookmark, Client, Error, Section};

glib::wrapper! {
    pub struct Window(ObjectSubclass<imp::Window>)
        @extends adw::ApplicationWindow, gtk::ApplicationWindow, gtk::Window, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Native,
            gtk::Root, gtk::ShortcutManager, gtk::gio::ActionGroup, gtk::gio::ActionMap;
}

impl Window {
    pub fn new(app: &adw::Application) -> Self {
        glib::Object::builder().property("application", app).build()
    }

    fn section_name(index: usize) -> &'static str {
        match index {
            0 => "Home",
            1 => "Liked",
            _ => "Archive",
        }
    }

    fn show_token_page(&self) {
        self.imp().stack.set_visible_child_name("token");
    }

    fn init_with_token(&self, token: String) {
        let _ = self.imp().client.set(Arc::new(Client::new(token)));
        self.imp().stack.set_visible_child_name("main");
        if let Some(first) = self.imp().sections_list.row_at_index(0) {
            self.imp().sections_list.select_row(Some(&first));
        }
        self.select_section(0);
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
                let saved =
                    gtk::gio::spawn_blocking(move || crate::secret::save_token(&token).is_ok())
                        .await
                        .unwrap_or(false);

                if saved {
                    let token = crate::secret::get_token().unwrap_or_default();
                    obj.init_with_token(token);
                } else {
                    let entry = &obj.imp().token_entry;
                    entry.set_sensitive(true);
                    entry.add_css_class("error");
                }
            }
        ));
    }

    fn select_section(&self, index: usize) {
        let imp = self.imp();
        imp.current_section.set(index);
        imp.content_title.set_title(Self::section_name(index));
        imp.lists_stack
            .set_visible_child_name(section_from_index(index).query_value());
        self.load_section(index);
    }

    fn load_section(&self, index: usize) {
        let imp = self.imp();
        let Some(client) = imp.client.get().cloned() else {
            return;
        };
        let view = imp.sections.borrow()[index].clone_view();
        view.set_loading();

        glib::spawn_future_local(async move {
            let result =
                gtk::gio::spawn_blocking(move || client.bookmarks(section_from_index(index))).await;
            match result {
                Ok(Ok(bookmarks)) => view.set_bookmarks(bookmarks),
                Ok(Err(error)) => view.set_error(&error.to_string()),
                Err(_) => view.set_error("The request task failed"),
            }
        });
    }

    fn setup_actions(&self) {
        let actions = gtk::gio::SimpleActionGroup::new();

        for name in [
            "open-bookmark",
            "archive-bookmark",
            "unarchive-bookmark",
            "like-bookmark",
            "unlike-bookmark",
            "delete-bookmark",
        ] {
            let action = gtk::gio::SimpleAction::new(name, None);
            let obj_weak = self.downgrade();
            let name = name.to_string();
            action.connect_activate(move |_, _| {
                if let Some(obj) = obj_weak.upgrade() {
                    obj.run_bookmark_action(&name);
                }
            });
            actions.add_action(&action);
        }

        self.insert_action_group("win", Some(&actions));
    }

    fn run_bookmark_action(&self, name: &str) {
        let imp = self.imp();
        let Some((section_index, row_index)) = imp.pending.get() else {
            return;
        };
        imp.pending.set(None);

        let Some(bookmark) = imp.sections.borrow()[section_index].bookmark_at(row_index) else {
            return;
        };

        match name {
            "open-bookmark" => self.open_in_browser(&bookmark),
            "delete-bookmark" => self.confirm_delete(section_index, row_index, bookmark.id),
            "archive-bookmark" => self.mutate(section_index, row_index, move |client| {
                client.archive(bookmark.id)
            }),
            "unarchive-bookmark" => self.mutate(section_index, row_index, move |client| {
                client.unarchive(bookmark.id)
            }),
            "like-bookmark" => self.mutate(section_index, row_index, move |client| {
                client.set_liked(bookmark.id, true)
            }),
            "unlike-bookmark" => self.mutate(section_index, row_index, move |client| {
                client.set_liked(bookmark.id, false)
            }),
            _ => {}
        }
    }

    fn open_in_browser(&self, bookmark: &Bookmark) {
        let Some(url) = bookmark.url.clone() else {
            return;
        };
        glib::spawn_future_local(async move {
            let _ = gtk::gio::spawn_blocking(move || {
                gtk::gio::AppInfo::launch_default_for_uri(&url, gtk::gio::AppLaunchContext::NONE)
            })
            .await;
        });
    }

    fn confirm_delete(&self, section_index: usize, row_index: usize, id: i64) {
        let dialog = adw::AlertDialog::builder()
            .heading("Delete bookmark?")
            .body("This permanently removes it from Instapaper. This cannot be undone.")
            .build();
        dialog.add_response("cancel", "Cancel");
        dialog.add_response("delete", "Delete");
        dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");

        let obj_weak = self.downgrade();
        dialog.connect_response(Some("delete"), move |_, _| {
            if let Some(obj) = obj_weak.upgrade() {
                obj.mutate(section_index, row_index, move |client| client.delete(id));
            }
        });

        dialog.present(Some(self));
    }

    fn mutate(
        &self,
        section_index: usize,
        row_index: usize,
        call: impl FnOnce(&Client) -> Result<(), Error> + Send + 'static,
    ) {
        let imp = self.imp();
        let Some(client) = imp.client.get().cloned() else {
            return;
        };
        let view = imp.sections.borrow()[section_index].clone_view();

        glib::spawn_future_local(async move {
            let result = gtk::gio::spawn_blocking(move || call(&client)).await;
            if matches!(result, Ok(Ok(()))) {
                view.remove_row(row_index);
            } else {
                view.set_error("The change could not be saved. Reload to try again.");
            }
        });
    }

    fn setup_key_navigation(&self) {
        let controller = gtk::EventControllerKey::new();
        let obj_weak = self.downgrade();
        controller.connect_key_pressed(move |_, keyval, _, _| {
            let Some(obj) = obj_weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            if obj.imp().stack.visible_child_name().as_deref() != Some("main") {
                return glib::Propagation::Proceed;
            }
            let delta = match keyval.name().as_deref() {
                Some("j") => 1,
                Some("k") => -1,
                _ => return glib::Propagation::Proceed,
            };
            let index = obj.imp().current_section.get();
            obj.imp().sections.borrow()[index].move_selection(delta);
            glib::Propagation::Stop
        });
        self.add_controller(controller);
    }

    fn show_bookmark_menu(
        &self,
        list: &gtk::ListBox,
        x: f64,
        y: f64,
        section_index: usize,
        row_index: usize,
    ) {
        let Some(bookmark) = self.imp().sections.borrow()[section_index].bookmark_at(row_index)
        else {
            return;
        };

        let menu = gtk::gio::Menu::new();

        let open_section = gtk::gio::Menu::new();
        open_section.append(Some("Open in Browser"), Some("win.open-bookmark"));
        menu.append_section(None, &open_section);

        let like_section = gtk::gio::Menu::new();
        if bookmark.liked {
            like_section.append(Some("Unlike"), Some("win.unlike-bookmark"));
        } else {
            like_section.append(Some("Like"), Some("win.like-bookmark"));
        }
        menu.append_section(None, &like_section);

        let move_section = gtk::gio::Menu::new();
        if section_from_index(section_index) == Section::Archive {
            move_section.append(Some("Move to Home"), Some("win.unarchive-bookmark"));
        } else {
            move_section.append(Some("Archive"), Some("win.archive-bookmark"));
        }
        menu.append_section(None, &move_section);

        let delete_section = gtk::gio::Menu::new();
        delete_section.append(Some("Delete…"), Some("win.delete-bookmark"));
        menu.append_section(None, &delete_section);

        let popover = gtk::PopoverMenu::from_model(Some(&menu));
        popover.set_parent(list);
        popover.connect_closed(|popover| popover.unparent());
        popover.set_pointing_to(Some(&gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
        popover.popup();
    }
}

fn section_from_index(index: usize) -> Section {
    match index {
        0 => Section::Home,
        1 => Section::Liked,
        _ => Section::Archive,
    }
}

pub(crate) struct SectionView {
    stack: gtk::Stack,
    list: gtk::ListBox,
    error_page: adw::StatusPage,
    bookmarks: std::rc::Rc<std::cell::RefCell<Vec<Bookmark>>>,
}

impl Clone for SectionView {
    fn clone(&self) -> Self {
        Self {
            stack: self.stack.clone(),
            list: self.list.clone(),
            error_page: self.error_page.clone(),
            bookmarks: self.bookmarks.clone(),
        }
    }
}

impl SectionView {
    fn new(index: usize, window: glib::WeakRef<Window>) -> Self {
        let list = gtk::ListBox::builder()
            .css_classes(["boxed-list"])
            .margin_top(12)
            .margin_bottom(24)
            .margin_start(12)
            .margin_end(12)
            .selection_mode(gtk::SelectionMode::Single)
            .build();

        let scrolled = gtk::ScrolledWindow::builder()
            .child(&list)
            .vexpand(true)
            .build();

        let spinner = gtk::Spinner::builder().spinning(true).build();
        let loading = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .valign(gtk::Align::Center)
            .build();
        loading.append(&gtk::Label::new(Some("Loading…")));
        loading.append(&spinner);

        let retry = gtk::Button::builder().label("Try Again").build();
        let error_page = adw::StatusPage::builder()
            .title("Could not load articles")
            .child(&retry)
            .build();
        let empty_page = adw::StatusPage::builder()
            .title("No articles")
            .description("Articles you save to Instapaper will appear here.")
            .build();

        let stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .vexpand(true)
            .build();
        stack.add_named(&loading, Some("loading"));
        stack.add_named(&scrolled, Some("content"));
        stack.add_named(&error_page, Some("error"));
        stack.add_named(&empty_page, Some("empty"));

        let retry_window = window.clone();
        retry.connect_clicked(move |_| {
            if let Some(window) = retry_window.upgrade() {
                window.select_section(index);
            }
        });

        let row_window = window.clone();
        list.connect_row_activated(move |_, row| {
            if let Some(window) = row_window.upgrade() {
                let section_index = window.imp().current_section.get();
                if let Some(bookmark) =
                    window.imp().sections.borrow()[section_index].bookmark_at(row.index() as usize)
                {
                    window.open_in_browser(&bookmark);
                }
            }
        });

        let gesture = gtk::GestureClick::new();
        gesture.set_button(3);
        let gesture_list = list.clone();
        let gesture_window = window.clone();
        gesture.connect_pressed(move |gesture, _, x, y| {
            let Some(window) = gesture_window.upgrade() else {
                return;
            };
            let Some(row) = gesture_list.row_at_y(y as i32) else {
                return;
            };
            let section_index = window.imp().current_section.get();
            let row_index = row.index() as usize;
            window.imp().pending.set(Some((section_index, row_index)));
            window.show_bookmark_menu(&gesture_list, x, y, section_index, row_index);
            gesture.set_state(gtk::EventSequenceState::Claimed);
        });
        list.add_controller(gesture);

        Self {
            stack,
            list,
            error_page,
            bookmarks: std::rc::Rc::new(std::cell::RefCell::new(Vec::new())),
        }
    }

    fn stack(&self) -> &gtk::Stack {
        &self.stack
    }

    fn clone_view(&self) -> Self {
        self.clone()
    }

    fn set_loading(&self) {
        self.stack.set_visible_child_name("loading");
    }

    fn set_error(&self, message: &str) {
        self.error_page.set_description(Some(message));
        self.stack.set_visible_child_name("error");
    }

    fn set_bookmarks(&self, bookmarks: Vec<Bookmark>) {
        *self.bookmarks.borrow_mut() = bookmarks;

        self.list.remove_all();

        let bookmarks = self.bookmarks.borrow();
        for bookmark in bookmarks.iter() {
            let row = adw::ActionRow::builder()
                .title(glib::markup_escape_text(&bookmark.display_title()))
                .subtitle(host_of(bookmark.url.as_deref()))
                .activatable(true)
                .build();
            self.list.append(&row);
        }

        let page = if bookmarks.is_empty() {
            "empty"
        } else {
            "content"
        };
        self.stack.set_visible_child_name(page);
    }

    fn bookmark_at(&self, index: usize) -> Option<Bookmark> {
        self.bookmarks.borrow().get(index).cloned()
    }

    fn remove_row(&self, index: usize) {
        self.bookmarks.borrow_mut().remove(index);
        if let Some(row) = self.list.row_at_index(index as i32) {
            self.list.remove(&row);
        }
        if self.bookmarks.borrow().is_empty() {
            self.stack.set_visible_child_name("empty");
        }
    }

    fn move_selection(&self, delta: i32) {
        let count = self.bookmarks.borrow().len() as i32;
        if count == 0 {
            return;
        }
        let current = self
            .list
            .selected_row()
            .map(|row| row.index())
            .unwrap_or(if delta > 0 { -1 } else { count });
        let next = (current + delta).clamp(0, count - 1);
        if let Some(row) = self.list.row_at_index(next) {
            self.list.select_row(Some(&row));
            row.grab_focus();
        }
    }
}

fn host_of(url: Option<&str>) -> String {
    let Some(url) = url else {
        return String::new();
    };
    let without_scheme = url.split_once("://").map(|(_, rest)| rest).unwrap_or(url);
    without_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or(without_scheme)
        .to_string()
}
