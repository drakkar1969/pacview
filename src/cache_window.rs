use std::fs;

use gtk::{glib, gio, gdk};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::clone;

use rayon::{iter::{IntoParallelRefIterator, ParallelIterator}, slice::ParallelSliceMut};

use crate::cache_object::CacheObject;
use crate::utils::open_containing_folder;

//------------------------------------------------------------------------------
// MODULE: CacheWindow
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/github/PacView/ui/cache_window.ui")]
    pub struct CacheWindow {
        #[template_child]
        pub(super) header_sub_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) search_entry: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub(super) open_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) copy_button: TemplateChild<gtk::Button>,

        #[template_child]
        pub(super) view: TemplateChild<gtk::ColumnView>,
        #[template_child]
        pub(super) model: TemplateChild<gio::ListStore>,
        #[template_child]
        pub(super) selection: TemplateChild<gtk::SingleSelection>,
        #[template_child]
        pub(super) search_filter: TemplateChild<gtk::StringFilter>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for CacheWindow {
        const NAME: &'static str = "CacheWindow";
        type Type = super::CacheWindow;
        type ParentType = adw::Window;

        fn class_init(klass: &mut Self::Class) {
            CacheObject::ensure_type();

            klass.bind_template();

            // Find key binding
            klass.add_binding(gdk::Key::F, gdk::ModifierType::CONTROL_MASK, |window| {
                let imp = window.imp();

                if !imp.search_entry.has_focus() {
                    imp.search_entry.grab_focus();
                }

                glib::Propagation::Stop
            });

            // Open key binding
            klass.add_binding(gdk::Key::O, gdk::ModifierType::CONTROL_MASK, |window| {
                let imp = window.imp();

                if imp.open_button.is_sensitive() {
                    imp.open_button.emit_clicked();
                }

                glib::Propagation::Stop
            });

            // Copy key binding
            klass.add_binding(gdk::Key::C, gdk::ModifierType::CONTROL_MASK, |window| {
                let imp = window.imp();

                if imp.copy_button.is_sensitive() {
                    imp.copy_button.emit_clicked();
                }

                glib::Propagation::Stop
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for CacheWindow {
        //---------------------------------------
        // Constructor
        //---------------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_widgets();
            obj.setup_controllers();
            obj.setup_signals();
        }
    }

    impl WidgetImpl for CacheWindow {}
    impl WindowImpl for CacheWindow {}
    impl AdwWindowImpl for CacheWindow {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: CacheWindow
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct CacheWindow(ObjectSubclass<imp::CacheWindow>)
    @extends adw::Window, gtk::Window, gtk::Widget,
    @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl CacheWindow {
    //---------------------------------------
    // New function
    //---------------------------------------
    pub fn new(parent: &impl IsA<gtk::Window>) -> Self {
        glib::Object::builder()
            .property("transient-for", parent)
            .build()
    }

    //---------------------------------------
    // Setup widgets
    //---------------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Set search entry key capture widget
        imp.search_entry.set_key_capture_widget(Some(&imp.view.get()));

        // Set initial focus on view
        imp.view.grab_focus();
    }

    //---------------------------------------
    // Setup controllers
    //---------------------------------------
    fn setup_controllers(&self) {
        // Create shortcut controller
        let controller = gtk::ShortcutController::new();
        controller.set_propagation_phase(gtk::PropagationPhase::Capture);

        // Close window shortcut
        controller.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("Escape"),
            Some(gtk::NamedAction::new("window.close"))
        ));

        // Add shortcut controller to window
        self.add_controller(controller);
    }

    //---------------------------------------
    // Setup signals
    //---------------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // Search entry search started signal
        imp.search_entry.connect_search_started(|entry| {
            if !entry.has_focus() {
                entry.grab_focus();
            }
        });

        // Search entry search changed signal
        imp.search_entry.connect_search_changed(clone!(
            #[weak] imp,
            move |entry| {
                imp.search_filter.set_search(Some(&entry.text()));
            }
        ));

        // Open button clicked signal
        imp.open_button.connect_clicked(clone!(
            #[weak] imp,
            move |_| {
                let item = imp.selection.selected_item()
                    .and_downcast::<CacheObject>()
                    .expect("Could not downcast to 'CacheObject'");

                open_containing_folder(&item.filename());
            }
        ));

        // Copy button clicked signal
        imp.copy_button.connect_clicked(clone!(
            #[weak(rename_to = window)] self,
            #[weak] imp,
            move |_| {
                let body = imp.selection.iter::<glib::Object>().flatten()
                    .map(|item| {
                        let cache = item
                            .downcast::<CacheObject>()
                            .expect("Could not downcast to 'CacheObject'");

                        format!("|{file}|", file=cache.filename())
                    })
                    .collect::<Vec<String>>()
                    .join("\n");

                window.clipboard().set_text(&
                    format!("## Cache Files\n|File|\n|---|\n{body}")
                );
            }
        ));

        // Selection items changed signal
        imp.selection.connect_items_changed(clone!(
            #[weak] imp,
            move |selection, _, _, _| {
                let n_items = selection.n_items();

                imp.header_sub_label.set_label(&format!("{n_items} file{}", if n_items != 1 { "s" } else { "" }));

                imp.copy_button.set_sensitive(n_items > 0);
            }
        ));
    }

    //---------------------------------------
    // Clear window
    //---------------------------------------
    pub fn remove_all(&self) {
        self.imp().model.remove_all();
    }

    //---------------------------------------
    // Show window
    //---------------------------------------
    pub fn show(&self, cache_dirs: &[String]) {
        let imp = self.imp();

        self.present();

        // Populate if necessary
        if imp.model.n_items() == 0 {
            // Get cache files
            let mut cache_files = cache_dirs.par_iter()
                .flat_map(|dir| {
                    let read_dir = fs::read_dir(dir);

                    read_dir.map(|read_dir| {
                        read_dir.into_iter()
                            .flatten()
                            .map(|entry| entry.path().display().to_string())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
                })
                .collect::<Vec<_>>();

            cache_files.par_sort_unstable();

            // Populate column view
            imp.model.splice(0, 0, &cache_files.iter()
                .map(|file| CacheObject::new(file))
                .collect::<Vec<CacheObject>>()
            );
        }
    }
}
