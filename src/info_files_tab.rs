use std::cell::RefCell;
use std::fmt::Write as _;

use gtk::subclass::prelude::*;
use gtk::prelude::*;
use gtk::{glib, gio};
use glib::clone;

use crate::pkg_object::PkgObject;
use crate::utils::AppInfoExt;

//------------------------------------------------------------------------------
// MODULE: InfoFilesTab
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::InfoFilesTab)]
    #[template(resource = "/com/github/PacView/ui/info_files_tab.ui")]
    pub struct InfoFilesTab {
        #[template_child]
        pub(super) header_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) count_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) search_entry: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub(super) filter_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub(super) open_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) copy_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) view: TemplateChild<gtk::ColumnView>,
        #[template_child]
        pub(super) model: TemplateChild<gio::ListStore>,
        #[template_child]
        pub(super) filter_model: TemplateChild<gtk::FilterListModel>,
        #[template_child]
        pub(super) selection: TemplateChild<gtk::SingleSelection>,
        #[template_child]
        pub(super) search_filter: TemplateChild<gtk::StringFilter>,
        #[template_child]
        pub(super) folder_filter: TemplateChild<gtk::CustomFilter>,
        #[template_child]
        pub(super) paused_status: TemplateChild<adw::StatusPage>,

        #[property(get, set)]
        pkg_name: RefCell<String>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for InfoFilesTab {
        const NAME: &'static str = "InfoFilesTab";
        type Type = super::InfoFilesTab;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for InfoFilesTab {
        //---------------------------------------
        // Constructor
        //---------------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_signals();
            obj.setup_widgets();
        }
    }
    impl WidgetImpl for InfoFilesTab {}
    impl BoxImpl for InfoFilesTab {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: InfoFilesTab
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct InfoFilesTab(ObjectSubclass<imp::InfoFilesTab>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl InfoFilesTab {
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

        // Filter button toggled signal
        imp.filter_button.connect_toggled(clone!(
            #[weak] imp,
            move |_| {
                imp.folder_filter.changed(gtk::FilterChange::Different);
            }
        ));

        // Open button clicked signal
        imp.open_button.connect_clicked(clone!(
            #[weak] imp,
            move |_| {
                let file = imp.selection.selected_item()
                    .and_downcast::<gtk::StringObject>()
                    .expect("Failed to downcast to 'StringObject'")
                    .string();

                glib::spawn_future_local(async move {
                    AppInfoExt::open_with_default_app(&file).await;
                });
            }
        ));

        // Copy button clicked signal
        imp.copy_button.connect_clicked(clone!(
            #[weak(rename_to = tab)] self,
            move |_| {
                let mut output = String::new();

                let _ = writeln!(output, "## {}\n|Files|\n|---|", tab.pkg_name());

                for obj in tab.imp().selection.iter::<glib::Object>()
                    .flatten()
                    .filter_map(|item| item.downcast::<gtk::StringObject>().ok()) {
                        let _ = writeln!(output, "{}", obj.string());
                    }

                tab.clipboard().set_text(&output);
            }
        ));

        // View activate signal
        imp.view.connect_activate(clone!(
            #[weak] imp,
            move |_, _| {
                if imp.open_button.is_sensitive() {
                    imp.open_button.emit_clicked();
                }
            }
        ));

        // Selection items changed signal
        imp.selection.connect_items_changed(clone!(
            #[weak] imp,
            move |selection, _, _, _| {
                let n_items = selection.n_items();

                imp.count_label.set_label(&n_items.to_string());
                imp.open_button.set_sensitive(n_items > 0);
                imp.copy_button.set_sensitive(n_items > 0);
            }
        ));
    }

    //---------------------------------------
    // Setup widgets
    //---------------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Set search entry key capture widget
        imp.search_entry.set_key_capture_widget(Some(&imp.view.get()));

        // Set folder filter function
        imp.folder_filter.set_filter_func(clone!(
            #[weak] imp,
            #[upgrade_or] false,
            move |item| {
                if imp.filter_button.is_active() {
                    true
                } else {
                    let obj = item
                        .downcast_ref::<gtk::StringObject>()
                        .expect("Failed to downcast to 'StringObject'");

                    !obj.string().ends_with('/')
                }
            }
        ));

        // Add keyboard shortcut to cancel search
        let shortcut = gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("Escape"),
            Some(gtk::CallbackAction::new(clone!(
                #[weak] imp,
                #[upgrade_or] glib::Propagation::Proceed,
                move |_, _| {
                    imp.search_entry.set_text("");
                    imp.view.grab_focus();

                    glib::Propagation::Stop
                }
            )))
        );

        let controller = gtk::ShortcutController::new();
        controller.add_shortcut(shortcut);

        imp.search_entry.add_controller(controller);
    }

    //---------------------------------------
    // Pause view function
    //---------------------------------------
    pub fn pause_view(&self) {
        let imp = self.imp();

        imp.paused_status.set_visible(true);
        imp.model.remove_all();
    }

    //---------------------------------------
    // Update view function
    //---------------------------------------
    pub fn update_view(&self, pkg: &PkgObject) {
        let imp = self.imp();

        imp.paused_status.set_visible(false);

        // Populate view
        let files_list: Vec<gtk::StringObject> = pkg.files().iter()
            .map(|file| gtk::StringObject::new(file))
            .collect();

        imp.model.splice(0, imp.model.n_items(), &files_list);

        self.set_pkg_name(pkg.name());
    }
}
