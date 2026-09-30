mod imp {
    use std::cell::{Cell, RefCell};
    use std::sync::{Arc, OnceLock};

    use adw::prelude::*;
    use adw::subclass::prelude::*;
    use gtk::glib;

    use super::ReaderWidgets;
    use crate::instapaper::{Bookmark, Client, Section};

    #[derive(gtk::CompositeTemplate)]
    #[template(resource = "/net/hardscrabble/oceans-ink/ui/window.ui")]
    pub struct Window {
        #[template_child]
        pub toast_overlay: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub token_entry: TemplateChild<adw::PasswordEntryRow>,
        #[template_child]
        pub save_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub sections_list: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub count_home: TemplateChild<gtk::Label>,
        #[template_child]
        pub count_liked: TemplateChild<gtk::Label>,
        #[template_child]
        pub count_archive: TemplateChild<gtk::Label>,
        #[template_child]
        pub lists_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub content_title: TemplateChild<adw::WindowTitle>,
        #[template_child]
        pub shortcuts_dialog: TemplateChild<adw::ShortcutsDialog>,
        #[template_child]
        pub content_nav: TemplateChild<adw::NavigationView>,
        #[template_child]
        pub page_controls: TemplateChild<gtk::Box>,
        #[template_child]
        pub page_first: TemplateChild<gtk::Button>,
        #[template_child]
        pub page_prev: TemplateChild<gtk::Button>,
        #[template_child]
        pub page_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub page_next: TemplateChild<gtk::Button>,
        #[template_child]
        pub page_last: TemplateChild<gtk::Button>,
        pub(crate) reader: RefCell<Option<ReaderWidgets>>,
        pub(crate) reader_current: RefCell<Option<(usize, usize, Bookmark)>>,
        pub thumbnails: std::sync::Arc<crate::thumbnail_cache::ThumbnailCache>,
        pub webview: RefCell<Option<webkit6::WebView>>,
        pub webview_base_uri: RefCell<Option<String>>,
        pub(crate) sections: RefCell<Vec<super::SectionView>>,
        pub current_section: Cell<usize>,
        pub pending: Cell<Option<(usize, usize)>>,
        pub client: Arc<OnceLock<Arc<Client>>>,
    }

    impl Default for Window {
        fn default() -> Self {
            Self {
                toast_overlay: TemplateChild::default(),
                stack: TemplateChild::default(),
                token_entry: TemplateChild::default(),
                save_button: TemplateChild::default(),
                sections_list: TemplateChild::default(),
                count_home: TemplateChild::default(),
                count_liked: TemplateChild::default(),
                count_archive: TemplateChild::default(),
                lists_stack: TemplateChild::default(),
                content_title: TemplateChild::default(),
                shortcuts_dialog: TemplateChild::default(),
                content_nav: TemplateChild::default(),
                page_controls: TemplateChild::default(),
                page_first: TemplateChild::default(),
                page_prev: TemplateChild::default(),
                page_label: TemplateChild::default(),
                page_next: TemplateChild::default(),
                page_last: TemplateChild::default(),
                reader: RefCell::new(None),
                reader_current: RefCell::new(None),
                thumbnails: std::sync::Arc::new(crate::thumbnail_cache::ThumbnailCache::open()),
                webview: RefCell::new(None),
                webview_base_uri: RefCell::new(None),
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

            self.token_entry.connect_activate(glib::clone!(
                #[weak]
                obj,
                move |_| obj.save_token_clicked()
            ));

            self.sections_list.connect_row_activated(glib::clone!(
                #[weak]
                obj,
                move |_, row| obj.select_section(row.index() as usize)
            ));

            self.page_first.connect_clicked(glib::clone!(
                #[weak]
                obj,
                move |_| obj.go_first_page()
            ));
            self.page_prev.connect_clicked(glib::clone!(
                #[weak]
                obj,
                move |_| obj.go_prev_page()
            ));
            self.page_next.connect_clicked(glib::clone!(
                #[weak]
                obj,
                move |_| obj.go_next_page()
            ));
            self.page_last.connect_clicked(glib::clone!(
                #[weak]
                obj,
                move |_| obj.go_last_page()
            ));

            obj.setup_actions();
            obj.setup_key_navigation();

            obj.connect_realize(|_| {
                if let Some(display) = gtk::gdk::Display::default() {
                    let theme = gtk::IconTheme::for_display(&display);
                    theme.add_resource_path("/net/hardscrabble/oceans-ink/icons");
                }
            });

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
use webkit6::prelude::*;

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

    fn show_toast(&self, message: &str) {
        self.imp().toast_overlay.add_toast(adw::Toast::new(message));
    }

    fn show_token_page(&self) {
        self.imp().stack.set_visible_child_name("token");
    }

    fn init_with_token(&self, token: String) {
        let _ = self.imp().client.set(Arc::new(Client::new(token)));
        self.imp().stack.set_visible_child_name("main");
        self.refresh_counts();
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
        if self.reader_visible() {
            imp.content_nav.pop();
        }
        imp.current_section.set(index);
        if let Some(row) = imp.sections_list.row_at_index(index as i32) {
            imp.sections_list.select_row(Some(&row));
        }
        imp.content_title.set_title(Self::section_name(index));
        imp.lists_stack
            .set_visible_child_name(section_from_index(index).query_value());
        self.load_section(index);
        self.update_page_controls();
    }

    fn current_view(&self) -> SectionView {
        let imp = self.imp();
        imp.sections.borrow()[imp.current_section.get()].clone_view()
    }

    fn update_page_controls(&self) {
        let imp = self.imp();
        let view = self.current_view();
        let page = view.page();
        let pages = view.page_count();
        imp.page_controls.set_visible(pages > 1);
        imp.page_label
            .set_text(&format!("{} / {}", page + 1, pages));
        imp.page_first.set_sensitive(page > 0);
        imp.page_prev.set_sensitive(page > 0);
        imp.page_next.set_sensitive(page + 1 < pages);
        imp.page_last.set_sensitive(page + 1 < pages);
    }

    fn go_first_page(&self) {
        self.go_to_page(0);
    }

    fn go_prev_page(&self) {
        let page = self.current_view().page();
        self.go_to_page(page.saturating_sub(1));
    }

    fn go_next_page(&self) {
        let page = self.current_view().page();
        self.go_to_page(page + 1);
    }

    fn go_last_page(&self) {
        let pages = self.current_view().page_count();
        self.go_to_page(pages.saturating_sub(1));
    }

    fn go_to_page(&self, page: u64) {
        let view = self.current_view();
        let clamped = page.min(view.page_count().saturating_sub(1));
        if clamped != view.page() {
            view.set_page(clamped);
            view.scroll_to_top();
            self.load_section(self.imp().current_section.get());
        }
    }

    fn load_section(&self, index: usize) {
        let imp = self.imp();
        let Some(client) = imp.client.get().cloned() else {
            return;
        };
        let view = imp.sections.borrow()[index].clone_view();
        let offset = crate::instapaper::offset_for_page(view.page());
        view.set_loading();
        let obj_weak = self.downgrade();
        let thumbnails = imp.thumbnails.clone();

        let list_client = client.clone();
        glib::spawn_future_local(async move {
            let result = gtk::gio::spawn_blocking(move || {
                list_client.bookmarks(section_from_index(index), offset)
            })
            .await;
            if let Some(obj) = obj_weak.upgrade() {
                match result {
                    Ok(Ok(page)) => {
                        let last = crate::instapaper::page_count(page.total).saturating_sub(1);
                        if page.bookmarks.is_empty() && view.page() > last {
                            view.set_total(page.total);
                            view.set_page(last);
                            obj.load_section(index);
                            return;
                        }
                        view.set_total(page.total);
                        view.set_bookmarks(page.bookmarks, (*client).clone(), thumbnails.clone());
                        obj.set_count_label(index, page.total);
                        obj.update_page_controls();
                    }
                    Ok(Err(error)) => view.set_error(&error.to_string()),
                    Err(_) => view.set_error("The request task failed"),
                }
            }
        });
    }

    fn set_count_label(&self, index: usize, count: u64) {
        let label = match index {
            0 => &self.imp().count_home,
            1 => &self.imp().count_liked,
            _ => &self.imp().count_archive,
        };
        label.set_text(&format_count(count));
    }

    fn refresh_counts(&self) {
        let Some(client) = self.imp().client.get().cloned() else {
            return;
        };
        let obj_weak = self.downgrade();
        glib::spawn_future_local(async move {
            let counts = gtk::gio::spawn_blocking(move || {
                [
                    (0usize, Section::Home),
                    (1, Section::Liked),
                    (2, Section::Archive),
                ]
                .into_iter()
                .map(|(index, section)| (index, client.count(section)))
                .collect::<Vec<_>>()
            })
            .await;
            if let Some(obj) = obj_weak.upgrade() {
                for (index, count) in counts.iter().flatten() {
                    if let Ok(count) = count {
                        obj.set_count_label(*index, *count);
                        obj.imp().sections.borrow()[*index].set_total(*count);
                    }
                }
                obj.update_page_controls();
            }
        });
    }

    fn setup_actions(&self) {
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
            self.add_action(&action);
        }

        let about = gtk::gio::SimpleAction::new("about", None);
        let about_weak = self.downgrade();
        about.connect_activate(move |_, _| {
            if let Some(obj) = about_weak.upgrade() {
                obj.show_about();
            }
        });
        self.add_action(&about);

        let shortcuts = gtk::gio::SimpleAction::new("keyboard-shortcuts", None);
        let shortcuts_weak = self.downgrade();
        shortcuts.connect_activate(move |_, _| {
            if let Some(obj) = shortcuts_weak.upgrade() {
                obj.show_shortcuts();
            }
        });
        self.add_action(&shortcuts);

        let select_section =
            gtk::gio::SimpleAction::new("select-section", Some(glib::VariantTy::INT32));
        let select_weak = self.downgrade();
        select_section.connect_activate(move |_, parameter| {
            let index = parameter
                .and_then(|value| value.get::<i32>())
                .unwrap_or(0)
                .clamp(0, 2) as usize;
            if let Some(obj) = select_weak
                .upgrade()
                .filter(|obj| obj.imp().current_section.get() != index)
            {
                obj.select_section(index)
            }
        });
        self.add_action(&select_section);

        if let Some(app) = self.application() {
            app.set_accels_for_action("win.keyboard-shortcuts", &["<Ctrl>question"]);
            for (index, accel) in ["<Ctrl>1", "<Ctrl>2", "<Ctrl>3"].iter().enumerate() {
                app.set_accels_for_action(&format!("win.select-section({index})"), &[accel]);
            }
        }
    }

    fn show_about(&self) {
        adw::AboutDialog::builder()
            .application_name("Oceans Ink")
            .application_icon("net.hardscrabble.oceans-ink")
            .version(env!("CARGO_PKG_VERSION"))
            .developer_name("Maxwell Jacobson")
            .website("https://github.com/maxjacobson/oceans-ink")
            .comments(
                "An unofficial Instapaper client for GNOME, vibecoded just for fun.\n\n\
                 A fish swims in the sea. While the sea is, in a certain sense, contained \
                 within the fish. Oh, what am I to think? What the writing of a thousand \
                 lifetimes could not explain, if all the forest's trees were pens, and all \
                 the oceans ink?",
            )
            .build()
            .present(Some(self));
    }

    fn show_shortcuts(&self) {
        self.imp().shortcuts_dialog.present(Some(self));
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
            "delete-bookmark" => self.confirm_delete(section_index, row_index, bookmark.id, false),
            "archive-bookmark" => self.mutate(
                section_index,
                row_index,
                MutationPlan {
                    mutation: Mutation::RemoveRow,
                    undo: Some(UndoSpec {
                        section_index,
                        reader_like_state: None,
                        call: Box::new(move |client| client.unarchive(bookmark.id)),
                    }),
                    after: None,
                },
                "Archived",
                move |client: &Client| client.archive(bookmark.id),
            ),
            "unarchive-bookmark" => self.mutate(
                section_index,
                row_index,
                MutationPlan {
                    mutation: Mutation::RemoveRow,
                    undo: Some(UndoSpec {
                        section_index,
                        reader_like_state: None,
                        call: Box::new(move |client| client.archive(bookmark.id)),
                    }),
                    after: None,
                },
                "Moved to home",
                move |client: &Client| client.unarchive(bookmark.id),
            ),
            "like-bookmark" => self.mutate(
                section_index,
                row_index,
                MutationPlan {
                    mutation: Mutation::SetLiked(true),
                    undo: Some(UndoSpec {
                        section_index,
                        reader_like_state: Some(false),
                        call: Box::new(move |client| client.set_liked(bookmark.id, false)),
                    }),
                    after: None,
                },
                "Liked",
                move |client: &Client| client.set_liked(bookmark.id, true),
            ),
            "unlike-bookmark" => self.mutate(
                section_index,
                row_index,
                MutationPlan {
                    mutation: Mutation::SetLiked(false),
                    undo: Some(UndoSpec {
                        section_index,
                        reader_like_state: Some(true),
                        call: Box::new(move |client| client.set_liked(bookmark.id, true)),
                    }),
                    after: None,
                },
                "Unliked",
                move |client: &Client| client.set_liked(bookmark.id, false),
            ),
            _ => {}
        }
    }

    fn open_in_browser(&self, bookmark: &Bookmark) {
        let url = bookmark.reader_url();
        self.open_uri(url);
    }

    fn open_uri(&self, url: String) {
        let launcher = gtk::UriLauncher::new(&url);
        glib::spawn_future_local(glib::clone!(
            #[weak(rename_to = obj)]
            self,
            async move {
                if let Err(error) = launcher.launch_future(Some(&obj)).await {
                    let dialog = adw::AlertDialog::builder()
                        .heading("Could not open the article")
                        .body(error.to_string())
                        .build();
                    dialog.add_response("ok", "OK");
                    dialog.set_default_response(Some("ok"));
                    dialog.present(Some(&obj));
                }
            }
        ));
    }

    fn confirm_delete(
        &self,
        section_index: usize,
        row_index: usize,
        id: i64,
        return_to_list: bool,
    ) {
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
                let after: Option<AfterMutation> = if return_to_list {
                    Some(advance_reader_after(section_index, row_index))
                } else {
                    None
                };
                obj.mutate(
                    section_index,
                    row_index,
                    MutationPlan {
                        mutation: Mutation::RemoveRow,
                        undo: None,
                        after,
                    },
                    "Bookmark deleted",
                    move |client| client.delete(id),
                );
            }
        });

        dialog.present(Some(self));
    }

    fn mutate(
        &self,
        section_index: usize,
        row_index: usize,
        plan: MutationPlan,
        success_toast: &str,
        call: impl FnOnce(&Client) -> Result<(), Error> + Send + 'static,
    ) {
        let imp = self.imp();
        let Some(client) = imp.client.get().cloned() else {
            return;
        };
        let undo_client = client.clone();
        let view = imp.sections.borrow()[section_index].clone_view();
        let obj_weak_for_counts = self.downgrade();
        let success_toast = success_toast.to_string();
        let MutationPlan {
            mutation,
            undo,
            after,
        } = plan;

        glib::spawn_future_local(async move {
            let result = gtk::gio::spawn_blocking(move || call(&client)).await;
            if matches!(result, Ok(Ok(()))) {
                let removed_row = matches!(mutation, Mutation::RemoveRow);
                match mutation {
                    Mutation::RemoveRow => view.remove_row(row_index),
                    Mutation::SetLiked(liked) => view.update_liked(row_index, liked),
                }
                if removed_row
                    && view.row_count() == 0
                    && view.page() > 0
                    && let Some(obj) = obj_weak_for_counts.upgrade()
                {
                    obj.load_section(section_index);
                }
                if let (Some(obj), Some(after)) = (obj_weak_for_counts.upgrade(), after) {
                    after(&obj, true);
                }
                if let Some(obj) = obj_weak_for_counts.upgrade() {
                    obj.refresh_counts();
                    match undo {
                        Some(undo) => {
                            let toast = adw::Toast::new(&success_toast);
                            toast.set_button_label(Some("Undo"));
                            let undo_window = obj_weak_for_counts.clone();
                            let undo_cell = std::cell::RefCell::new(Some(undo));
                            let undo_client_cell = std::cell::RefCell::new(Some(undo_client));
                            toast.connect_button_clicked(move |_| {
                                let Some(undo) = undo_cell.borrow_mut().take() else {
                                    return;
                                };
                                let Some(undo_client) = undo_client_cell.borrow_mut().take() else {
                                    return;
                                };
                                if let Some(obj) = undo_window.upgrade() {
                                    let section_index = undo.section_index;
                                    glib::spawn_future_local(async move {
                                        let result = gtk::gio::spawn_blocking(move || {
                                            (undo.call)(&undo_client)
                                        })
                                        .await;
                                        if matches!(result, Ok(Ok(()))) {
                                            obj.load_section(section_index);
                                            obj.refresh_counts();
                                            if let Some(liked) = undo.reader_like_state {
                                                obj.sync_reader_like_after_undo(liked);
                                            }
                                            if removed_row {
                                                obj.pop_reader_if_showing(section_index);
                                            }
                                        }
                                    });
                                }
                            });
                            obj.imp().toast_overlay.add_toast(toast);
                        }
                        None => obj.show_toast(&success_toast),
                    }
                }
            } else {
                if let (Some(obj), Some(after)) = (obj_weak_for_counts.upgrade(), after) {
                    after(&obj, false);
                }
                view.set_error("The change could not be saved. Reload to try again.");
            }
        });
    }

    fn setup_key_navigation(&self) {
        let controller = gtk::EventControllerKey::new();
        let obj_weak = self.downgrade();
        controller.connect_key_pressed(move |_, keyval, _, modifier| {
            let Some(obj) = obj_weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            if obj.imp().stack.visible_child_name().as_deref() != Some("main") {
                return glib::Propagation::Proceed;
            }
            let visible_tag = obj.imp().content_nav.visible_page_tag();
            let tag = visible_tag.as_deref();
            let ctrl = modifier.contains(gtk::gdk::ModifierType::CONTROL_MASK);
            match (keyval.name().as_deref(), ctrl) {
                (Some("j"), false) if tag == Some("list") => {
                    let index = obj.imp().current_section.get();
                    obj.imp().sections.borrow()[index].move_selection(1);
                    glib::Propagation::Stop
                }
                (Some("k"), false) if tag == Some("list") => {
                    let index = obj.imp().current_section.get();
                    obj.imp().sections.borrow()[index].move_selection(-1);
                    glib::Propagation::Stop
                }
                (Some("j"), false) if tag == Some("reader") => {
                    obj.reader_move(1);
                    glib::Propagation::Stop
                }
                (Some("k"), false) if tag == Some("reader") => {
                    obj.reader_move(-1);
                    glib::Propagation::Stop
                }
                (Some("1"), true) => obj.switch_to_section(0),
                (Some("2"), true) => obj.switch_to_section(1),
                (Some("3"), true) => obj.switch_to_section(2),
                (Some("l"), false) => {
                    obj.toggle_like();
                    glib::Propagation::Stop
                }
                (Some("y"), false) => {
                    obj.toggle_archive();
                    glib::Propagation::Stop
                }
                (Some("BackSpace"), _) => {
                    obj.delete_active();
                    glib::Propagation::Stop
                }
                (Some("b"), false) => {
                    if let Some((_, _, bookmark)) = obj.active_bookmark() {
                        let url = bookmark
                            .url
                            .clone()
                            .unwrap_or_else(|| bookmark.reader_url());
                        obj.open_uri(url);
                    }
                    glib::Propagation::Stop
                }
                (Some("question"), true) => {
                    obj.show_shortcuts();
                    glib::Propagation::Stop
                }
                (Some("q"), true) => {
                    if let Some(app) = obj.application() {
                        app.quit();
                    }
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        });
        self.add_controller(controller);
    }

    fn switch_to_section(&self, index: usize) -> glib::Propagation {
        if self.imp().current_section.get() != index {
            self.select_section(index);
        }
        glib::Propagation::Stop
    }

    fn selected_bookmark(&self) -> Option<(usize, usize, Bookmark)> {
        let imp = self.imp();
        let section_index = imp.current_section.get();
        let view = &imp.sections.borrow()[section_index];
        let row_index = view.selected_index()?;
        let bookmark = view.bookmark_at(row_index)?;
        Some((section_index, row_index, bookmark))
    }

    fn reader_visible(&self) -> bool {
        self.imp().content_nav.visible_page_tag().as_deref() == Some("reader")
    }

    fn active_bookmark(&self) -> Option<(usize, usize, Bookmark)> {
        if self.reader_visible() {
            return self.imp().reader_current.borrow().clone();
        }
        self.selected_bookmark()
    }

    fn sync_reader_like_after_undo(&self, liked: bool) {
        if !self.reader_visible() {
            return;
        }
        if let Some((_, _, bookmark)) = self.imp().reader_current.borrow_mut().as_mut() {
            bookmark.liked = liked;
        }
        self.refresh_reader_like_state(liked);
    }

    fn pop_reader_if_showing(&self, section_index: usize) {
        if !self.reader_visible() {
            return;
        }
        let showing = self
            .imp()
            .reader_current
            .borrow()
            .as_ref()
            .is_some_and(|(reader_section, _, _)| *reader_section == section_index);
        if showing {
            self.imp().content_nav.pop();
            self.imp().sections.borrow()[section_index].restore_scroll();
        }
    }

    fn refresh_reader_like_state(&self, liked: bool) {
        let reader = self.reader();
        reader.like_icon.set_visible(true);
        reader
            .like_icon
            .set_icon_name(Some("oceans-ink-heart-filled-symbolic"));
        reader.like_icon.set_css_classes(if liked {
            &["oi-heart", "oi-liked"][..]
        } else {
            &["oi-heart"][..]
        });
        reader
            .like_button
            .set_tooltip_text(Some(if liked { "Unlike (l)" } else { "Like (l)" }));
    }

    fn toggle_like(&self) {
        let Some((section_index, row_index, bookmark)) = self.active_bookmark() else {
            return;
        };
        let in_reader = self.reader_visible();
        let id = bookmark.id;
        let liked = !bookmark.liked;
        if in_reader {
            self.refresh_reader_like_state(liked);
            if let Some((_, _, bookmark)) = self.imp().reader_current.borrow_mut().as_mut() {
                bookmark.liked = liked;
            }
        }
        if !liked && section_from_index(section_index) == Section::Liked {
            self.mutate(
                section_index,
                row_index,
                MutationPlan {
                    mutation: Mutation::RemoveRow,
                    undo: Some(UndoSpec {
                        section_index,
                        reader_like_state: Some(true),
                        call: Box::new(move |client: &Client| client.set_liked(id, true)),
                    }),
                    after: None,
                },
                "Unliked",
                move |client: &Client| client.set_liked(id, false),
            );
            if in_reader {
                self.imp().content_nav.pop();
                self.imp().sections.borrow()[section_index].restore_scroll();
            }
        } else {
            self.mutate(
                section_index,
                row_index,
                MutationPlan {
                    mutation: Mutation::SetLiked(liked),
                    undo: Some(UndoSpec {
                        section_index,
                        reader_like_state: Some(!liked),
                        call: Box::new(move |client: &Client| client.set_liked(id, !liked)),
                    }),
                    after: None,
                },
                if liked { "Liked" } else { "Unliked" },
                move |client: &Client| client.set_liked(id, liked),
            );
        }
    }

    fn toggle_archive(&self) {
        let Some((section_index, row_index, bookmark)) = self.active_bookmark() else {
            return;
        };
        let id = bookmark.id;
        let after: Option<AfterMutation> = if self.reader_visible() {
            Some(advance_reader_after(section_index, row_index))
        } else {
            None
        };
        if section_from_index(section_index) == Section::Archive {
            self.mutate(
                section_index,
                row_index,
                MutationPlan {
                    mutation: Mutation::RemoveRow,
                    undo: Some(UndoSpec {
                        section_index,
                        reader_like_state: None,
                        call: Box::new(move |client: &Client| client.archive(id)),
                    }),
                    after,
                },
                "Moved to home",
                move |client: &Client| client.unarchive(id),
            );
        } else {
            self.mutate(
                section_index,
                row_index,
                MutationPlan {
                    mutation: Mutation::RemoveRow,
                    undo: Some(UndoSpec {
                        section_index,
                        reader_like_state: None,
                        call: Box::new(move |client: &Client| client.unarchive(id)),
                    }),
                    after,
                },
                "Archived",
                move |client: &Client| client.archive(id),
            );
        }
    }

    fn delete_active(&self) {
        let Some((section_index, row_index, bookmark)) = self.active_bookmark() else {
            return;
        };
        let return_to_list = self.reader_visible();
        self.confirm_delete(section_index, row_index, bookmark.id, return_to_list);
    }

    fn open_reader(&self, section_index: usize, row_index: usize, bookmark: &Bookmark) {
        self.imp().sections.borrow()[section_index].save_anchor();
        self.show_in_reader(section_index, row_index, bookmark, true);
    }

    fn advance_reader(&self, section_index: usize, row_index: usize, bookmark: &Bookmark) {
        self.imp().sections.borrow()[section_index].select_index(row_index);
        self.show_in_reader(section_index, row_index, bookmark, false);
    }

    fn reader_move(&self, delta: i32) {
        let Some((section_index, row_index, _)) = self.imp().reader_current.borrow().clone() else {
            return;
        };
        let view = self.imp().sections.borrow()[section_index].clone_view();
        let target = row_index as isize + delta as isize;
        if target >= 0
            && let Some(bookmark) = view.bookmark_at(target as usize)
        {
            self.advance_reader(section_index, target as usize, &bookmark);
            return;
        }
        if delta > 0 && view.can_next_page() {
            self.load_page_and_advance_reader(section_index, view.page() + 1, false);
        } else if delta < 0 && view.can_prev_page() {
            self.load_page_and_advance_reader(section_index, view.page() - 1, true);
        }
    }

    fn reader_advance_or_pop(&self, section_index: usize, row_index: usize) {
        let view = self.imp().sections.borrow()[section_index].clone_view();
        if let Some(bookmark) = view.bookmark_at(row_index) {
            self.advance_reader(section_index, row_index, &bookmark);
            return;
        }
        if view.can_next_page() {
            self.load_page_and_advance_reader(section_index, view.page() + 1, false);
            return;
        }
        self.imp().content_nav.pop();
        view.restore_scroll();
    }

    fn load_page_and_advance_reader(&self, section_index: usize, page: u64, from_end: bool) {
        let Some(client) = self.imp().client.get().cloned() else {
            return;
        };
        let view = self.imp().sections.borrow()[section_index].clone_view();
        let obj_weak = self.downgrade();
        let thumbnails = self.imp().thumbnails.clone();
        let list_client = client.clone();
        glib::spawn_future_local(async move {
            let result = gtk::gio::spawn_blocking(move || {
                list_client.bookmarks(
                    section_from_index(section_index),
                    crate::instapaper::offset_for_page(page),
                )
            })
            .await;
            if let Some(obj) = obj_weak.upgrade() {
                match result {
                    Ok(Ok(page_data)) if !page_data.bookmarks.is_empty() => {
                        view.set_total(page_data.total);
                        view.set_page(page);
                        view.set_bookmarks(
                            page_data.bookmarks,
                            (*client).clone(),
                            thumbnails.clone(),
                        );
                        obj.update_page_controls();
                        let row = if from_end {
                            view.row_count().saturating_sub(1)
                        } else {
                            0
                        };
                        if let Some(bookmark) = view.bookmark_at(row) {
                            obj.advance_reader(section_index, row, &bookmark);
                        }
                    }
                    _ => {
                        obj.load_section(section_index);
                    }
                }
            }
        });
    }

    fn show_in_reader(
        &self,
        section_index: usize,
        row_index: usize,
        bookmark: &Bookmark,
        push: bool,
    ) {
        let imp = self.imp();
        let reader = self.reader();
        reader.like_icon.set_visible(true);
        reader
            .like_icon
            .set_icon_name(Some("oceans-ink-heart-filled-symbolic"));
        reader.like_icon.set_css_classes(if bookmark.liked {
            &["oi-heart", "oi-liked"][..]
        } else {
            &["oi-heart"][..]
        });
        reader.like_button.set_tooltip_text(Some(if bookmark.liked {
            "Unlike (l)"
        } else {
            "Like (l)"
        }));
        reader.archive_icon.set_icon_name(Some(
            if section_from_index(section_index) == Section::Archive {
                "edit-undo-symbolic"
            } else {
                "oceans-ink-archive-symbolic"
            },
        ));
        reader.archive_button.set_tooltip_text(Some(
            if section_from_index(section_index) == Section::Archive {
                "Move to home (y)"
            } else {
                "Archive (y)"
            },
        ));
        *imp.reader_current.borrow_mut() = Some((section_index, row_index, bookmark.clone()));

        if is_video(bookmark.url.as_deref()) {
            reader.external_heading.set_text(&bookmark.display_title());
            reader.external_thumb.set_visible(false);
            reader.stack.set_visible_child_name("external");
            if push {
                imp.content_nav.push_by_tag("reader");
            }
            self.load_reader_thumbnail(bookmark.image.clone());
            return;
        }

        reader.stack.set_visible_child_name("loading");
        if push {
            imp.content_nav.push_by_tag("reader");
        }

        let Some(client) = imp.client.get().cloned() else {
            return;
        };
        let id = bookmark.id;
        let base_uri = bookmark.url.clone();
        let obj_weak = self.downgrade();
        glib::spawn_future_local(async move {
            let result = gtk::gio::spawn_blocking(move || client.article(id)).await;
            if let Some(obj) = obj_weak.upgrade() {
                match result {
                    Ok(Ok(article)) => obj.show_article(article, base_uri),
                    Ok(Err(error)) => obj.show_article_error(&error.to_string()),
                    Err(_) => obj.show_article_error("The request task failed"),
                }
            }
        });
    }

    fn load_reader_thumbnail(&self, image_url: Option<String>) {
        let reader = self.reader();
        load_reader_thumbnail_impl(self, &reader, image_url);
    }

    fn show_article(&self, article: crate::instapaper::ParsedArticle, base_uri: Option<String>) {
        let imp = self.imp();
        let Some(body) = article.content.body else {
            self.show_article_error("This article has no readable content.");
            return;
        };
        let title = article
            .metadata
            .title
            .clone()
            .filter(|title| !title.is_empty())
            .or_else(|| {
                imp.reader_current
                    .borrow()
                    .as_ref()
                    .map(|(_, _, bookmark)| bookmark.display_title())
            })
            .unwrap_or_else(|| "Untitled".to_string());
        let escaped_title = glib::markup_escape_text(&title);
        let html = format!("<h1 class=\"oi-article-title\">{escaped_title}</h1>{body}");
        *imp.webview_base_uri.borrow_mut() = base_uri.clone();
        let webview = self.webview();
        webview.load_html(&html, base_uri.as_deref());
        self.reader().stack.set_visible_child_name("content");
    }

    fn show_article_error(&self, message: &str) {
        let reader = self.reader();
        let title = self
            .imp()
            .reader_current
            .borrow()
            .as_ref()
            .map(|(_, _, bookmark)| bookmark.display_title())
            .unwrap_or_else(|| "Could not load article".to_string());
        reader.error.set_title(&title);
        reader
            .error
            .set_description(Some(&format!("Could not load the article. {message}")));
        reader.stack.set_visible_child_name("error");
    }

    fn webview(&self) -> webkit6::WebView {
        let imp = self.imp();
        if let Some(existing) = imp.webview.borrow().as_ref() {
            return existing.clone();
        }

        let webview = webkit6::WebView::builder()
            .hexpand(true)
            .vexpand(true)
            .build();
        let stylesheet = webkit6::UserStyleSheet::new(
            "body { max-width: 42rem; margin: 0 auto; padding: 1rem 1.5rem 3rem; font-family: sans-serif; line-height: 1.6; } img, video { max-width: 100%; height: auto; } .oi-article-title { margin: 0 0 1rem; line-height: 1.25; }",
            webkit6::UserContentInjectedFrames::AllFrames,
            webkit6::UserStyleLevel::User,
            &[] as &[&str],
            &[] as &[&str],
        );
        if let Some(manager) = webview.user_content_manager() {
            manager.add_style_sheet(&stylesheet);
        }
        let window_weak = self.downgrade();
        webview.connect_decide_policy(move |_, decision, decision_type| {
            if decision_type != webkit6::PolicyDecisionType::NavigationAction {
                return false;
            }
            let Some(navigation) = decision.downcast_ref::<webkit6::NavigationPolicyDecision>()
            else {
                return false;
            };
            let mut action = navigation.navigation_action().unwrap();
            let Some(request) = action.request() else {
                return false;
            };
            let Some(uri) = request.uri() else {
                return false;
            };
            let uri = uri.to_string();
            if !uri.starts_with("http://") && !uri.starts_with("https://") {
                return false;
            }
            let same_page = window_weak.upgrade().is_some_and(|window| {
                let base = window.imp().webview_base_uri.borrow().clone();
                Some(without_fragment(&uri)) == base.as_deref().map(without_fragment)
            });
            if same_page {
                return false;
            }
            decision.ignore();
            let launcher = gtk::UriLauncher::new(&uri);
            glib::spawn_future_local(async move {
                let _ = launcher.launch_future(None::<&gtk::Window>).await;
            });
            true
        });

        self.reader().container.append(&webview);
        *imp.webview.borrow_mut() = Some(webview.clone());
        webview
    }

    fn reader(&self) -> ReaderWidgets {
        if let Some(existing) = self.imp().reader.borrow().as_ref() {
            return existing.clone();
        }

        let header = adw::HeaderBar::new();

        let container = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .build();

        let loading = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .valign(gtk::Align::Center)
            .spacing(12)
            .build();
        loading.append(&gtk::Label::new(Some("Loading…")));
        loading.append(&gtk::Spinner::builder().spinning(true).build());

        let error = adw::StatusPage::builder()
            .title("Could not load article")
            .build();

        let external_button = gtk::Button::builder()
            .label("Open in Browser")
            .css_classes(["suggested-action", "pill"])
            .halign(gtk::Align::Center)
            .margin_bottom(24)
            .build();
        external_button.set_cursor_from_name(Some("pointer"));
        let external_window = self.downgrade();
        external_button.connect_clicked(move |_| {
            if let Some(obj) = external_window.upgrade() {
                let url = obj
                    .imp()
                    .reader_current
                    .borrow()
                    .clone()
                    .and_then(|(_, _, b)| b.url);
                if let Some(url) = url {
                    obj.open_uri(url);
                }
            }
        });
        let external_heading = gtk::Label::builder()
            .wrap(true)
            .xalign(0.5)
            .css_classes(["title-2"])
            .margin_top(16)
            .build();
        let external_thumb = gtk::Picture::builder()
            .can_shrink(true)
            .hexpand(true)
            .visible(false)
            .build();
        let external_note = gtk::Label::builder()
            .label("This article is a video. Watch it in your browser instead.")
            .css_classes(["dim-label"])
            .wrap(true)
            .build();
        let thumb_button = gtk::Button::builder()
            .child(&external_thumb)
            .css_classes(["flat"])
            .build();
        thumb_button.set_cursor_from_name(Some("pointer"));
        let thumb_window = self.downgrade();
        thumb_button.connect_clicked(move |_| {
            if let Some(obj) = thumb_window.upgrade() {
                let url = obj
                    .imp()
                    .reader_current
                    .borrow()
                    .clone()
                    .and_then(|(_, _, bookmark)| bookmark.url);
                if let Some(url) = url {
                    obj.open_uri(url);
                }
            }
        });

        let external = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .valign(gtk::Align::Center)
            .spacing(12)
            .build();
        external.append(&external_heading);
        external.append(&thumb_button);
        external.append(&external_note);
        external.append(&external_button);

        let stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .vexpand(true)
            .build();
        stack.add_named(&loading, Some("loading"));
        stack.add_named(&container, Some("content"));
        stack.add_named(&error, Some("error"));
        stack.add_named(&external, Some("external"));

        let like_icon = gtk::Image::builder()
            .icon_name("oceans-ink-heart-filled-symbolic")
            .css_classes(["oi-heart"])
            .visible(false)
            .build();
        let header_like_button = gtk::Button::builder()
            .child(&like_icon)
            .css_classes(["flat", "cursor-pointer"])
            .tooltip_text("Like (l)")
            .build();
        header_like_button.set_cursor_from_name(Some("pointer"));
        let header_like_window = self.downgrade();
        header_like_button.connect_clicked(move |_| {
            if let Some(obj) = header_like_window.upgrade() {
                obj.toggle_like();
            }
        });

        let archive_icon = gtk::Image::builder()
            .icon_name("oceans-ink-archive-symbolic")
            .build();
        let header_archive_button = gtk::Button::builder()
            .child(&archive_icon)
            .css_classes(["flat", "cursor-pointer"])
            .tooltip_text("Archive (y)")
            .build();
        header_archive_button.set_cursor_from_name(Some("pointer"));
        let header_archive_window = self.downgrade();
        header_archive_button.connect_clicked(move |_| {
            if let Some(obj) = header_archive_window.upgrade() {
                obj.toggle_archive();
            }
        });

        let delete_icon = gtk::Image::builder()
            .icon_name("user-trash-symbolic")
            .build();
        let header_delete_button = gtk::Button::builder()
            .child(&delete_icon)
            .css_classes(["flat", "cursor-pointer"])
            .tooltip_text("Delete (Backspace)")
            .build();
        header_delete_button.set_cursor_from_name(Some("pointer"));
        let header_delete_window = self.downgrade();
        header_delete_button.connect_clicked(move |_| {
            if let Some(obj) = header_delete_window.upgrade() {
                obj.delete_active();
            }
        });

        let open_icon = gtk::Image::builder()
            .icon_name("oceans-ink-globe-symbolic")
            .build();
        let header_open_button = gtk::Button::builder()
            .child(&open_icon)
            .css_classes(["flat", "cursor-pointer"])
            .tooltip_text("Open in browser (b)")
            .build();
        header_open_button.set_cursor_from_name(Some("pointer"));
        let header_open_window = self.downgrade();
        header_open_button.connect_clicked(move |_| {
            if let Some(obj) = header_open_window.upgrade() {
                let url = obj
                    .imp()
                    .reader_current
                    .borrow()
                    .clone()
                    .and_then(|(_, _, bookmark)| bookmark.url);
                if let Some(url) = url {
                    obj.open_uri(url);
                }
            }
        });

        header.pack_end(&header_delete_button);
        header.pack_end(&header_archive_button);
        header.pack_end(&header_like_button);
        header.pack_end(&header_open_button);

        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&header);
        toolbar.set_content(Some(&stack));

        let page = adw::NavigationPage::builder()
            .tag("reader")
            .title("Article")
            .child(&toolbar)
            .build();
        self.imp().content_nav.add(&page);

        let widgets = ReaderWidgets {
            page,
            stack,
            container,
            error,
            like_button: header_like_button,
            like_icon,
            archive_button: header_archive_button,
            archive_icon,
            delete_button: header_delete_button,
            open_button: header_open_button,
            external_heading,
            external_thumb,
            external_button,
        };
        *self.imp().reader.borrow_mut() = Some(widgets.clone());
        widgets
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

        let popover = gtk::Popover::builder().css_classes(["menu"]).build();
        let items = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .margin_top(6)
            .margin_bottom(6)
            .build();

        let add_item = |label: &str, shortcut: Option<&str>| -> gtk::Button {
            let button = menu_item(label, shortcut);
            items.append(&button);
            button
        };

        let open_item = add_item("Open in browser", None);
        let open_window = self.downgrade();
        let open_popover = popover.downgrade();
        open_item.connect_clicked(move |_| {
            if let Some(p) = open_popover.upgrade() {
                p.popdown();
            }
            if let Some(obj) = open_window.upgrade() {
                obj.run_bookmark_action("open-bookmark");
            }
        });
        items.append(&open_item);

        let like_item = add_item(if bookmark.liked { "Unlike" } else { "Like" }, Some("l"));
        let like_window = self.downgrade();
        let like_popover = popover.downgrade();
        like_item.connect_clicked(move |_| {
            if let Some(p) = like_popover.upgrade() {
                p.popdown();
            }
            if let Some(obj) = like_window.upgrade() {
                obj.run_bookmark_action(if bookmark.liked {
                    "unlike-bookmark"
                } else {
                    "like-bookmark"
                });
            }
        });
        items.append(&like_item);

        let archive_item = add_item(
            if section_from_index(section_index) == Section::Archive {
                "Move to home"
            } else {
                "Archive"
            },
            Some("y"),
        );
        let archive_window = self.downgrade();
        let archive_popover = popover.downgrade();
        archive_item.connect_clicked(move |_| {
            if let Some(p) = archive_popover.upgrade() {
                p.popdown();
            }
            if let Some(obj) = archive_window.upgrade() {
                obj.run_bookmark_action(if section_from_index(section_index) == Section::Archive {
                    "unarchive-bookmark"
                } else {
                    "archive-bookmark"
                });
            }
        });
        items.append(&archive_item);

        let delete_item = add_item("Delete…", Some("BackSpace"));
        let delete_window = self.downgrade();
        let delete_popover = popover.downgrade();
        delete_item.connect_clicked(move |_| {
            if let Some(p) = delete_popover.upgrade() {
                p.popdown();
            }
            if let Some(obj) = delete_window.upgrade() {
                obj.run_bookmark_action("delete-bookmark");
            }
        });
        items.append(&delete_item);

        popover.set_child(Some(&items));
        popover.set_parent(list);
        popover.connect_closed(|popover| popover.unparent());
        popover.set_pointing_to(Some(&gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
        popover.popup();
    }
}

enum Mutation {
    RemoveRow,
    SetLiked(bool),
}

type UndoCall = Box<dyn FnOnce(&Client) -> Result<(), Error> + Send + 'static>;

type AfterMutation = Box<dyn FnOnce(&Window, bool)>;

struct MutationPlan {
    mutation: Mutation,
    undo: Option<UndoSpec>,
    after: Option<AfterMutation>,
}

struct UndoSpec {
    section_index: usize,
    reader_like_state: Option<bool>,
    call: UndoCall,
}

fn section_from_index(index: usize) -> Section {
    match index {
        0 => Section::Home,
        1 => Section::Liked,
        _ => Section::Archive,
    }
}

fn advance_reader_after(section_index: usize, row_index: usize) -> AfterMutation {
    Box::new(move |window, success| {
        if success {
            window.reader_advance_or_pop(section_index, row_index);
        } else {
            window.imp().content_nav.pop();
            window.imp().sections.borrow()[section_index].restore_scroll();
        }
    })
}

pub(crate) struct SectionView {
    section: Section,
    stack: gtk::Stack,
    list: gtk::ListBox,
    scroller: gtk::ScrolledWindow,
    anchored_id: std::cell::Cell<Option<i64>>,
    anchored_index: std::cell::Cell<usize>,
    page: std::cell::Cell<u64>,
    total: std::cell::Cell<u64>,
    error_page: adw::StatusPage,
    bookmarks: std::rc::Rc<std::cell::RefCell<Vec<Bookmark>>>,
    icons: std::rc::Rc<std::cell::RefCell<Vec<gtk::Image>>>,
}

impl Clone for SectionView {
    fn clone(&self) -> Self {
        Self {
            section: self.section,
            stack: self.stack.clone(),
            list: self.list.clone(),
            scroller: self.scroller.clone(),
            anchored_id: self.anchored_id.clone(),
            anchored_index: self.anchored_index.clone(),
            page: self.page.clone(),
            total: self.total.clone(),
            error_page: self.error_page.clone(),
            bookmarks: self.bookmarks.clone(),
            icons: self.icons.clone(),
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

        let retry = gtk::Button::builder().label("Try again").build();
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
                    window.open_reader(section_index, row.index() as usize, &bookmark);
                }
            }
        });

        let gesture = gtk::GestureClick::new();
        gesture.set_button(3);
        let gesture_list = list.clone();
        let gesture_window = window.clone();
        gesture.connect_pressed(move |gesture, _, _, _| {
            gesture.set_state(gtk::EventSequenceState::Claimed);
        });
        gesture.connect_released(move |_, _, x, y| {
            let Some(window) = gesture_window.upgrade() else {
                return;
            };
            let Some(row) = gesture_list.row_at_y(y as i32) else {
                return;
            };
            let section_index = window.imp().current_section.get();
            let row_index = row.index() as usize;
            gesture_list.select_row(Some(&row));
            row.grab_focus();
            window.imp().pending.set(Some((section_index, row_index)));
            window.show_bookmark_menu(&gesture_list, x, y, section_index, row_index);
        });
        list.add_controller(gesture);

        Self {
            section: section_from_index(index),
            stack,
            list,
            scroller: scrolled.clone(),
            anchored_id: std::cell::Cell::new(None),
            anchored_index: std::cell::Cell::new(0),
            page: std::cell::Cell::new(0),
            total: std::cell::Cell::new(0),
            error_page,
            bookmarks: std::rc::Rc::new(std::cell::RefCell::new(Vec::new())),
            icons: std::rc::Rc::new(std::cell::RefCell::new(Vec::new())),
        }
    }

    fn stack(&self) -> &gtk::Stack {
        &self.stack
    }

    fn save_anchor(&self) {
        let value = self.scroller.vadjustment().value();
        let anchored_index = self
            .list
            .row_at_y(value as i32)
            .map(|row| row.index() as usize);
        self.anchored_index.set(anchored_index.unwrap_or(0));
        self.anchored_id
            .set(anchored_index.and_then(|index| self.bookmarks.borrow().get(index).map(|b| b.id)));
    }

    fn restore_scroll(&self) {
        let target = self.anchor_offset();
        let adjustment = self.scroller.vadjustment();
        glib::idle_add_local_once({
            let adjustment = adjustment.clone();
            move || adjustment.set_value(target)
        });
        glib::timeout_add_local_once(std::time::Duration::from_millis(350), move || {
            adjustment.set_value(target)
        });
    }

    fn anchor_offset(&self) -> f64 {
        let bookmarks = self.bookmarks.borrow();
        let target_index = self
            .anchored_id
            .get()
            .and_then(|id| bookmarks.iter().position(|bookmark| bookmark.id == id))
            .unwrap_or_else(|| {
                self.anchored_index
                    .get()
                    .min(bookmarks.len().saturating_sub(1))
            });
        let Some(row) = self.list.row_at_index(target_index as i32) else {
            return 0.0;
        };
        let Some(bounds) = row.compute_bounds(&self.list) else {
            return 0.0;
        };
        bounds.y().max(0.0) as f64
    }

    fn clone_view(&self) -> Self {
        self.clone()
    }

    fn set_loading(&self) {
        self.stack.set_visible_child_name("loading");
    }

    fn page(&self) -> u64 {
        self.page.get()
    }

    fn set_page(&self, page: u64) {
        self.page.set(page);
    }

    fn set_total(&self, total: u64) {
        self.total.set(total);
    }

    fn page_count(&self) -> u64 {
        crate::instapaper::page_count(self.total.get())
    }

    fn can_prev_page(&self) -> bool {
        self.page.get() > 0
    }

    fn can_next_page(&self) -> bool {
        self.page.get() + 1 < self.page_count()
    }

    fn scroll_to_top(&self) {
        let adjustment = self.scroller.vadjustment();
        glib::idle_add_local_once(move || adjustment.set_value(0.0));
    }

    fn set_error(&self, message: &str) {
        self.error_page.set_description(Some(message));
        self.stack.set_visible_child_name("error");
    }

    fn set_bookmarks(
        &self,
        bookmarks: Vec<Bookmark>,
        client: crate::instapaper::Client,
        thumbnails: std::sync::Arc<crate::thumbnail_cache::ThumbnailCache>,
    ) {
        *self.bookmarks.borrow_mut() = bookmarks;
        self.icons.borrow_mut().clear();

        self.list.remove_all();

        let bookmarks = self.bookmarks.borrow();
        for bookmark in bookmarks.iter() {
            let row = adw::ActionRow::builder()
                .title(glib::markup_escape_text(&bookmark.display_title()))
                .subtitle(host_of(bookmark.url.as_deref()))
                .activatable(true)
                .build();
            let thumbnail = gtk::Image::builder()
                .icon_name("image-x-generic-symbolic")
                .pixel_size(96)
                .width_request(96)
                .height_request(96)
                .valign(gtk::Align::Center)
                .build();
            row.add_prefix(&thumbnail);
            if let Some(image_url) = bookmark.image.clone() {
                let thumbnail_weak = thumbnail.downgrade();
                let image_url = image_url.clone();
                match thumbnails.get(&image_url) {
                    Some(bytes) => {
                        apply_texture(&thumbnail, &bytes);
                    }
                    None => {
                        let thumbnails = thumbnails.clone();
                        let image_client = client.clone();
                        let fetch_url = image_url.clone();
                        glib::spawn_future_local(async move {
                            let bytes = gtk::gio::spawn_blocking(move || {
                                image_client.image_bytes(&fetch_url)
                            })
                            .await;
                            if let (Some(thumbnail), Ok(Ok(bytes))) =
                                (thumbnail_weak.upgrade(), bytes)
                            {
                                thumbnails.put(&image_url, &bytes);
                                apply_texture(&thumbnail, &bytes);
                            }
                        });
                    }
                }
            }
            let show_heart = bookmark.liked && self.section != Section::Liked;
            let icon = gtk::Image::builder()
                .icon_name("oceans-ink-heart-filled-symbolic")
                .css_classes(if bookmark.liked {
                    &["oi-heart", "oi-liked"][..]
                } else {
                    &["oi-heart"][..]
                })
                .visible(show_heart)
                .build();
            row.add_suffix(&icon);
            self.icons.borrow_mut().push(icon);
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

    fn row_count(&self) -> usize {
        self.bookmarks.borrow().len()
    }

    fn remove_row(&self, index: usize) {
        self.bookmarks.borrow_mut().remove(index);
        self.icons.borrow_mut().remove(index);
        if let Some(row) = self.list.row_at_index(index as i32) {
            self.list.remove(&row);
        }
        if self.bookmarks.borrow().is_empty() {
            self.stack.set_visible_child_name("empty");
        }
    }

    fn update_liked(&self, index: usize, liked: bool) {
        if let Some(bookmark) = self.bookmarks.borrow_mut().get_mut(index) {
            bookmark.liked = liked;
        }
        if let Some(icon) = self.icons.borrow().get(index) {
            icon.set_icon_name(Some("oceans-ink-heart-filled-symbolic"));
            icon.set_css_classes(if liked {
                &["oi-heart", "oi-liked"][..]
            } else {
                &["oi-heart"][..]
            });
            icon.set_visible(liked && self.section != Section::Liked);
        }
    }

    fn selected_index(&self) -> Option<usize> {
        self.list.selected_row().map(|row| row.index() as usize)
    }

    fn select_index(&self, index: usize) {
        if let Some(row) = self.list.row_at_index(index as i32) {
            self.list.select_row(Some(&row));
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

pub(crate) struct ReaderWidgets {
    pub page: adw::NavigationPage,
    pub stack: gtk::Stack,
    pub container: gtk::Box,
    pub error: adw::StatusPage,
    pub like_button: gtk::Button,
    pub like_icon: gtk::Image,
    pub archive_button: gtk::Button,
    pub archive_icon: gtk::Image,
    pub delete_button: gtk::Button,
    pub open_button: gtk::Button,
    pub external_heading: gtk::Label,
    pub external_thumb: gtk::Picture,

    pub external_button: gtk::Button,
}

impl Clone for ReaderWidgets {
    fn clone(&self) -> Self {
        Self {
            page: self.page.clone(),
            stack: self.stack.clone(),
            container: self.container.clone(),
            error: self.error.clone(),
            like_button: self.like_button.clone(),
            like_icon: self.like_icon.clone(),
            archive_button: self.archive_button.clone(),
            archive_icon: self.archive_icon.clone(),
            delete_button: self.delete_button.clone(),
            open_button: self.open_button.clone(),
            external_heading: self.external_heading.clone(),
            external_thumb: self.external_thumb.clone(),

            external_button: self.external_button.clone(),
        }
    }
}

fn menu_item(label: &str, shortcut: Option<&str>) -> gtk::Button {
    let text = gtk::Label::builder()
        .label(label)
        .halign(gtk::Align::Start)
        .xalign(0.0)
        .hexpand(true)
        .build();
    let box_ = gtk::Box::builder().spacing(12).build();
    box_.append(&text);
    if let Some(shortcut) = shortcut {
        box_.append(&adw::ShortcutLabel::new(shortcut));
    }
    gtk::Button::builder()
        .child(&box_)
        .css_classes(["flat"])
        .build()
}

fn format_count(count: u64) -> String {
    let digits = count.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (position, digit) in digits.chars().enumerate() {
        let remaining = digits.len() - position;
        if position > 0 && remaining.is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    grouped
}

fn apply_picture(picture: &gtk::Picture, bytes: &[u8]) {
    let glib_bytes = glib::Bytes::from_owned(bytes.to_vec());
    if let Ok(texture) = gtk::gdk::Texture::from_bytes(&glib_bytes) {
        picture.set_paintable(Some(&texture));
    }
}

fn apply_texture(thumbnail: &gtk::Image, bytes: &[u8]) {
    let glib_bytes = glib::Bytes::from_owned(bytes.to_vec());
    if let Ok(texture) = gtk::gdk::Texture::from_bytes(&glib_bytes) {
        thumbnail.set_paintable(Some(&texture));
    }
}

fn load_reader_thumbnail_impl(window: &Window, reader: &ReaderWidgets, image_url: Option<String>) {
    let Some(url) = image_url else {
        return;
    };
    match window.imp().thumbnails.get(&url) {
        Some(bytes) => {
            reader.external_thumb.set_visible(true);
            apply_picture(&reader.external_thumb, &bytes);
        }
        None => {
            let Some(client) = window.imp().client.get().cloned() else {
                return;
            };
            let cache = window.imp().thumbnails.clone();
            let thumb_weak = reader.external_thumb.downgrade();
            let fetch_url = url.clone();
            glib::spawn_future_local(async move {
                let bytes = gtk::gio::spawn_blocking(move || client.image_bytes(&fetch_url)).await;
                if let (Some(thumb), Ok(Ok(bytes))) = (thumb_weak.upgrade(), bytes) {
                    cache.put(&url, &bytes);
                    thumb.set_visible(true);
                    apply_picture(&thumb, &bytes);
                }
            });
        }
    }
}

fn is_video(url: Option<&str>) -> bool {
    matches!(
        url,
        Some(u)
            if u.contains("youtube.com/")
                || u.contains("youtu.be/")
                || u.contains("vimeo.com/")
    )
}

fn without_fragment(uri: &str) -> &str {
    uri.split('#').next().unwrap_or(uri)
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

#[cfg(test)]
mod tests {
    use super::{format_count, is_video};

    #[test]
    fn recognizes_video_hosts() {
        assert!(is_video(Some("https://www.youtube.com/watch?v=x")));
        assert!(is_video(Some("https://youtu.be/x")));
        assert!(is_video(Some("https://vimeo.com/123456")));
        assert!(!is_video(Some(
            "https://example.com/video-about-youtube.com"
        )));
        assert!(!is_video(Some("https://example.com/")));
    }

    #[test]
    fn groups_thousands_with_commas() {
        assert_eq!(format_count(0), "0");
        assert_eq!(format_count(999), "999");
        assert_eq!(format_count(1_000), "1,000");
        assert_eq!(format_count(12_345_678), "12,345,678");
    }
}
