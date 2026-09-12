use std::cell::{Cell, RefCell};
use std::fmt::Write as _;

use adw::subclass::prelude::*;
use adw::prelude::*;
use gtk::{glib, gio};
use glib::clone;

use crate::{
    pkg_object::PkgObject,
    utils::{Pacman, AppInfoExt}
};

//------------------------------------------------------------------------------
// ENUM: TabState
//------------------------------------------------------------------------------
#[derive(Default, Debug, Eq, PartialEq, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "TabState")]
pub enum TabState {
    Loading,
    #[default]
    Installed,
    Remote,
}

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
        pub(super) count_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) search_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub(super) search_bar: TemplateChild<gtk::SearchBar>,
        #[template_child]
        pub(super) search_entry: TemplateChild<gtk::SearchEntry>,

        #[template_child]
        pub(super) view: TemplateChild<gtk::ListView>,
        #[template_child]
        pub(super) model: TemplateChild<gio::ListStore>,
        #[template_child]
        pub(super) selection: TemplateChild<gtk::SingleSelection>,
        #[template_child]
        pub(super) search_filter: TemplateChild<gtk::StringFilter>,
        #[template_child]
        pub(super) folder_filter: TemplateChild<gtk::CustomFilter>,
        #[template_child]
        pub(super) loading_status: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub(super) empty_status: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub(super) remote_status: TemplateChild<adw::StatusPage>,

        #[property(get, set)]
        pkg_name: RefCell<String>,
        #[property(get, set, builder(TabState::default()))]
        state: Cell<TabState>,
        #[property(get, set)]
        show_folders: Cell<bool>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for InfoFilesTab {
        const NAME: &'static str = "InfoFilesTab";
        type Type = super::InfoFilesTab;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            // Install actions
            Self::install_actions(klass);
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
    impl BinImpl for InfoFilesTab {}

    impl InfoFilesTab {
        //---------------------------------------
        // Install actions
        //---------------------------------------
        fn install_actions(klass: &mut <Self as ObjectSubclass>::Class) {
            // Show folders property action
            klass.install_property_action("info.files-show-folders", "show-folders");

            // Open action
            klass.install_action_async("info.files-open", None, async |tab, _, _| {
                if let Some(file) = tab.imp().selection.selected_item()
                    .and_downcast::<gtk::StringObject>() {
                        let path = Pacman::config().read().unwrap().root_dir.clone() + &file.string();

                        AppInfoExt::open_with_default_app(&path).await;
                    }
            });

            // Copy action
            klass.install_action("info.files-copy", None, |tab, _, _| {
                let mut output = String::new();

                writeln!(output, "### {}\n|Files|\n|---|", tab.pkg_name()).unwrap();

                for obj in tab.imp().selection.iter::<glib::Object>()
                    .filter_map(|item| item.ok().and_downcast::<gtk::StringObject>()) {
                        writeln!(output, "{}", obj.string()).unwrap();
                    }

                tab.clipboard().set_text(&output);
            });
        }
    }
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: InfoFilesTab
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct InfoFilesTab(ObjectSubclass<imp::InfoFilesTab>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl InfoFilesTab {
    //---------------------------------------
    // Setup signals
    //---------------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // Search bar search mode enabled signal
        imp.search_bar.connect_search_mode_enabled_notify(clone!(
            #[weak] imp,
            move |bar| {
                if !bar.is_search_mode() {
                    imp.view.grab_focus();
                }
            }
        ));

        // Search entry search changed signal
        imp.search_entry.connect_search_changed(clone!(
            #[weak] imp,
            move |entry| {
                imp.search_filter.set_search(Some(&entry.text()));
            }
        ));

        // State property notify signal
        self.connect_state_notify(|tab| {
            let imp = tab.imp();

            match tab.state() {
                TabState::Loading => {
                    imp.search_button.set_sensitive(false);

                    imp.loading_status.set_visible(true);
                    imp.remote_status.set_visible(false);
                    imp.empty_status.set_visible(false);
                }
                TabState::Installed => {
                    imp.search_button.set_sensitive(true);

                    imp.loading_status.set_visible(false);
                    imp.remote_status.set_visible(false);
                }
                TabState::Remote => {
                    imp.search_button.set_sensitive(false);
                    imp.search_bar.set_search_mode(false);

                    imp.loading_status.set_visible(false);
                    imp.remote_status.set_visible(true);
                    imp.empty_status.set_visible(false);
                }
            }
        });

        // Show folders property notify signal
        self.connect_show_folders_notify(|tab| {
            tab.imp().folder_filter.changed(gtk::FilterChange::Different);
        });

        // View activate signal
        imp.view.connect_activate(clone!(
            #[weak(rename_to = tab)] self,
            move |_, _| {
                tab.activate_action("info.files-open", None).unwrap();
            }
        ));

        // Selection items changed signal
        imp.selection.connect_items_changed(clone!(
            #[weak(rename_to = tab)] self,
            move |selection, _, _, _| {
                let imp = tab.imp();

                let n_items = selection.n_items();

                imp.count_label.set_label(&n_items.to_string());

                imp.empty_status.set_visible(tab.state() == TabState::Installed && n_items == 0);

                tab.action_set_enabled("info.files-show-folders", n_items > 0);
                tab.action_set_enabled("info.files-open", n_items > 0);
                tab.action_set_enabled("info.files-copy", n_items > 0);
            }
        ));
    }

    //---------------------------------------
    // Setup widgets
    //---------------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Set search entry key capture widget
        imp.search_bar.set_key_capture_widget(Some(&imp.view.get()));

        // Bind search button state to search bar visibility
        imp.search_button.bind_property("active", &imp.search_bar.get(), "search-mode-enabled")
            .bidirectional()
            .sync_create()
            .build();

        // Set folder filter function
        imp.folder_filter.set_filter_func(clone!(
            #[weak(rename_to = tab)] self,
            #[upgrade_or] false,
            move |item| {
                if tab.show_folders() {
                    true
                } else {
                    let obj = item
                        .downcast_ref::<gtk::StringObject>()
                        .expect("Failed to downcast to 'StringObject'");

                    !obj.string().ends_with('/')
                }
            }
        ));
    }

    //---------------------------------------
    // Pause view function
    //---------------------------------------
    pub fn pause_view(&self) {
        self.set_state(TabState::Loading);

        self.imp().model.remove_all();
    }

    //---------------------------------------
    // Update view function
    //---------------------------------------
    pub fn update_view(&self, pkg: &PkgObject) {
        let imp = self.imp();

        if pkg.is_installed() {
            self.set_state(TabState::Installed);

            glib::spawn_future_local(clone!(
                #[weak] imp,
                #[weak] pkg,
                async move {
                    // Populate view
                    let files_list: Vec<gtk::StringObject> = pkg.files().iter()
                        .map(|file| gtk::StringObject::new(file))
                        .collect();

                    imp.model.splice(0, imp.model.n_items(), &files_list);

                    if imp.selection.n_items() == 0 {
                        imp.empty_status.set_visible(true);
                    }
                }
            ));
        } else {
            self.set_state(TabState::Remote);

            imp.model.remove_all();
        }

        self.set_pkg_name(pkg.name());
    }
}
