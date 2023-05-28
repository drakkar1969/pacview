use gtk::{gio, glib, gdk};
use adw::subclass::prelude::*;
use gtk::prelude::*;

use crate::pkg_object::PkgObject;
use crate::toggle_button::ToggleButton;

//------------------------------------------------------------------------------
// MODULE: DetailsWindow
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //-----------------------------------
    // Private structure
    //-----------------------------------
    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/github/PacView/ui/details_window.ui")]
    pub struct DetailsWindow {
        #[template_child]
        pub pkg_label: TemplateChild<gtk::Label>,

        #[template_child]
        pub content_stack: TemplateChild<gtk::Stack>,

        #[template_child]
        pub files_header_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub files_search_entry: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub files_open_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub files_copy_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub files_view: TemplateChild<gtk::ListView>,
        #[template_child]
        pub files_model: TemplateChild<gtk::StringList>,
        #[template_child]
        pub files_selection: TemplateChild<gtk::SingleSelection>,
        #[template_child]
        pub files_filter: TemplateChild<gtk::StringFilter>,
    }

    //-----------------------------------
    // Subclass
    //-----------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for DetailsWindow {
        const NAME: &'static str = "DetailsWindow";
        type Type = super::DetailsWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            ToggleButton::static_type();

            klass.bind_template();
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for DetailsWindow {
        //-----------------------------------
        // Constructor
        //-----------------------------------
        fn constructed(&self) {
            self.parent_constructed();
        }
    }

    impl WidgetImpl for DetailsWindow {}
    impl WindowImpl for DetailsWindow {}
    impl ApplicationWindowImpl for DetailsWindow {}
    impl AdwApplicationWindowImpl for DetailsWindow {}

    #[gtk::template_callbacks]
    impl DetailsWindow {
        //-----------------------------------
        // Stack button signal handler
        //-----------------------------------
        #[template_callback]
        fn on_stack_button_toggled(&self, button: ToggleButton) {
            if button.is_active() {
                self.content_stack.set_visible_child_name(&button.text().to_lowercase());
            }
        }

        //-----------------------------------
        // Open file manager helper function
        //-----------------------------------
        fn open_file_manager(&self, path: &str) {
            if let Some(desktop) = gio::AppInfo::default_for_type("inode/directory", true) {
                let path = format!("file://{}", path);

                desktop.launch_uris(&[&path], None::<&gio::AppLaunchContext>).unwrap();
            }
        }

        //-----------------------------------
        // Files page signal handlers
        //-----------------------------------
        #[template_callback]
        fn on_files_search_changed(&self, entry: &gtk::SearchEntry) {
            self.files_filter.set_search(Some(&entry.text()));
        }

        #[template_callback]
        fn on_files_open_button_clicked(&self) {
            if let Some(item) = self.files_selection.selected_item() {
                if let Some(file) = item.downcast_ref::<gtk::StringObject>() {
                    self.open_file_manager(&file.string());
                }
            }
        }

        #[template_callback]
        fn on_files_copy_button_clicked(&self) {
            let files_list: Vec<String> = IntoIterator::into_iter(0..self.files_selection.n_items())
            .map(|i| {
                let item: gtk::StringObject = self.files_selection.item(i).and_downcast().expect("Must be a StringObject");

                item.string().to_string()
            })
            .collect();

            let copy_text = files_list.join("\n");

            let clipboard = self.obj().clipboard();

            clipboard.set_text(&copy_text);
        }

        //-----------------------------------
        // Key press signal handler
        //-----------------------------------
        #[template_callback]
        fn on_key_pressed(&self, key: u32, _: u32, state: gdk::ModifierType) -> bool {
            if key == 65307 && state.is_empty() {
                self.obj().close();

                true
            } else {
                false
            }
        }
    }
}

//------------------------------------------------------------------------------
// PUBLIC IMPLEMENTATION: DetailsWindow
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct DetailsWindow(ObjectSubclass<imp::DetailsWindow>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow, adw::ApplicationWindow,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable,
                    gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl DetailsWindow {
    //-----------------------------------
    // Public new function
    //-----------------------------------
    pub fn new(pkg: Option<PkgObject>) -> Self {
        let win: Self = glib::Object::builder().build();

        if let Some(pkg) = pkg {
            win.setup_banner(&pkg);
            win.setup_files(&pkg);
        }

        win
    }

    //-----------------------------------
    // Setup banner
    //-----------------------------------
    fn setup_banner(&self, pkg: &PkgObject) {
        // Set package name in banner
        self.imp().pkg_label.set_label(&format!("{repo}/{name}", repo=pkg.repo_show(), name=pkg.name()));
    }

    //-----------------------------------
    // Setup files page
    //-----------------------------------
    fn setup_files(&self, pkg: &PkgObject) {
        let imp = self.imp();

        let files = pkg.files();

        // Set files search entry key capture widget
        imp.files_search_entry.set_key_capture_widget(Some(&imp.files_view.get().upcast::<gtk::Widget>()));

        // Bind files count to files header label
        imp.files_selection.bind_property("n-items", &imp.files_header_label.get(), "label")
            .transform_to(|_, n_items: u32| {
                Some(format!("Files ({})", n_items))
            })
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

        // Bind files count to files open/copy button states
        imp.files_selection.bind_property("n-items", &imp.files_open_button.get(), "sensitive")
            .transform_to(|_, n_items: u32| {
                Some(n_items != 0)
            })
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

        imp.files_selection.bind_property("n-items", &imp.files_copy_button.get(), "sensitive")
            .transform_to(|_, n_items: u32| {
                Some(n_items != 0)
            })
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

        // Populate files list
        imp.files_model.splice(0, 0, &files.iter().map(|s| s.as_str()).collect::<Vec<&str>>());
    }
}
