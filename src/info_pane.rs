use std::cell::{Cell, RefCell, OnceCell};
use std::marker::PhantomData;
use std::time::Duration;

use gtk::glib;
use adw::subclass::prelude::*;
use adw::prelude::*;
use glib::{clone, Variant, VariantTy};

use crate::{
    package_view::PackageView,
    info_details_tab::InfoDetailsTab,
    info_files_tab::{InfoFilesTab, TabState},
    history_list::HistoryList,
    pkg_object::PkgObject,
    source_dialog::SourceDialog,
    hash_dialog::HashDialog,
    traits::ListStoreFind
};

//------------------------------------------------------------------------------
// ENUM: PaneDisplayMode
//------------------------------------------------------------------------------
#[derive(Default, Debug, Eq, PartialEq, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "PaneDisplayMode")]
pub enum PaneDisplayMode {
    #[default]
    Normal,
    Collapsed,
    Narrow
}

//------------------------------------------------------------------------------
// MODULE: InfoPane
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::InfoPane)]
    #[template(resource = "/com/github/PacView/ui/info_pane.ui")]
    pub struct InfoPane {
        #[template_child]
        pub(super) tab_header_bar: TemplateChild<adw::HeaderBar>,
        #[template_child]
        pub(super) tab_switcher: TemplateChild<adw::ViewSwitcher>,
        #[template_child]
        pub(super) tab_header_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) prev_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) next_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) show_button: TemplateChild<gtk::ToggleButton>,

        #[template_child]
        pub(super) main_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) tab_stack: TemplateChild<adw::ViewStack>,

        #[template_child]
        pub(super) info_tab: TemplateChild<InfoDetailsTab>,
        #[template_child]
        pub(super) files_tab: TemplateChild<InfoFilesTab>,

        #[template_child]
        pub(super) tab_switcher_bar: TemplateChild<adw::ViewSwitcherBar>,

        #[property(get, set, builder(PaneDisplayMode::default()))]
        display_mode: Cell<PaneDisplayMode>,
        #[property(get = Self::pkg, set = Self::set_pkg, nullable)]
        pkg: PhantomData<Option<PkgObject>>,
        #[property(get, set, default = "info", construct)]
        active_tab: RefCell<String>,

        #[property(get, set)]
        package_view: OnceCell<PackageView>,

        pub(super) pkg_history: RefCell<HistoryList>,

        pub(super) update_delay_id: RefCell<Option<glib::SourceId>>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for InfoPane {
        const NAME: &'static str = "InfoPane";
        type Type = super::InfoPane;
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
    impl ObjectImpl for InfoPane {
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

    impl WidgetImpl for InfoPane {}
    impl BinImpl for InfoPane {}
    impl InfoPane {
        //---------------------------------------
        // Install actions
        //---------------------------------------
        fn install_actions(klass: &mut <Self as ObjectSubclass>::Class) {
            // Previous action
            klass.install_action("info.previous", None, |pane, _, _| {
                pane.imp().pkg_history.borrow().select_previous();

                pane.update_display();
            });

            // Next action
            klass.install_action("info.next", None, |pane, _, _| {
                pane.imp().pkg_history.borrow().select_next();

                pane.update_display();
            });

            // Show PKGBUILD action
            klass.install_action("info.show-pkgbuild", None, |pane, _, _| {
                if let Some(pkg) = pane.pkg() {
                    let parent = pane.root()
                        .and_downcast::<gtk::Window>()
                        .expect("Failed to downcast to 'GtkWindow'");

                    let source_dialog = SourceDialog::new(&parent, &pkg);

                    source_dialog.present(Some(pane));
                }
            });

            // Show hashes action
            klass.install_action("info.show-hashes", None, |pane, _, _| {
                if let Some(pkg) = pane.pkg() && pkg.validation().is_valid() {
                    let dialog = HashDialog::new(&pkg);

                    dialog.present(Some(pane));
                }
            });

            // Info row handle pkg link action
            klass.install_action("info.handle-pkg-link", Some(VariantTy::TUPLE), |pane, _, param| {
                let (pkg_name, pkg_version) = param
                    .and_then(Variant::get::<(String, Option<String>)>)
                    .expect("Failed to get tuple from variant");

                // Find link package in pacman databases or AUR search results
                let mut pkg_link = pkg_name.clone();
                
                if let Some(version) = pkg_version {
                   pkg_link += &version;
                }

                let pkg_model = pane.package_view().pkg_model();
                let aur_model = pane.package_view().aur_model();

                let new_pkg = PkgObject::find_satisfier(&pkg_link, &pkg_model)
                    .or_else(|| aur_model.find_with(|pkg: &PkgObject| pkg.name() == pkg_name))
                    .or_else(|| aur_model.find_with(|pkg: &PkgObject| {
                        pkg.provides().iter().any(|s| s == &pkg_link)
                    }));

                // If link package found and is different from current package
                if let Some(pkg) = new_pkg.filter(|pkg| pane.pkg().as_ref() != Some(pkg)) {
                    // If link package is in history, select it
                    // Otherwise append it after selected history package
                    pane.imp().pkg_history.borrow().select_or_append(pkg);

                    // Display link package
                    pane.update_display();
                }
            });
        }

        //---------------------------------------
        // Property getter/setter
        //---------------------------------------
        fn pkg(&self) -> Option<PkgObject> {
            self.pkg_history.borrow().selected_item()
        }

        fn set_pkg(&self, pkg: Option<PkgObject>) {
            self.main_stack.set_visible_child_name(
                if pkg.is_some() { "properties" } else { "empty" }
            );

            self.pkg_history.borrow().init(pkg);

            self.obj().update_display();
        }
    }
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: InfoPane
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct InfoPane(ObjectSubclass<imp::InfoPane>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl InfoPane {
    //---------------------------------------
    // Setup signals
    //---------------------------------------
    fn setup_signals(&self) {
        // Display mode property notify signal
        self.connect_display_mode_notify(|pane| {
            let imp = pane.imp();

            let mode = pane.display_mode();

            imp.show_button.set_visible(mode != PaneDisplayMode::Normal);

            if mode == PaneDisplayMode::Narrow {
                imp.tab_header_bar.set_title_widget(Some(&imp.tab_header_label.get()));
            } else {
                imp.tab_header_bar.set_title_widget(Some(&imp.tab_switcher.get()));
            }

            imp.tab_switcher_bar.set_reveal(mode == PaneDisplayMode::Narrow);
        });
    }

    //---------------------------------------
    // Setup widgets
    //---------------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Bind history list properties to widgets
        let history = imp.pkg_history.borrow();

        history.bind_property("len", &imp.prev_button.get(), "visible")
            .transform_to(|_, len: u32| Some(len > 1))
            .sync_create()
            .build();

        history.bind_property("len", &imp.next_button.get(), "visible")
            .transform_to(|_, len: u32| Some(len > 1))
            .sync_create()
            .build();

        history.bind_property("peek-previous", &imp.prev_button.get(), "sensitive")
            .sync_create()
            .build();

        history.bind_property("peek-next", &imp.next_button.get(), "sensitive")
            .sync_create()
            .build();

        // Bind active tab property to visible page
        self.bind_property("active-tab", &imp.tab_stack.get(), "visible-child-name")
            .sync_create()
            .build();
    }

    //---------------------------------------
    // Public update display function
    //---------------------------------------
    pub fn update_display(&self) {
        let imp = self.imp();

        // If package is not none, display it
        if let Some(pkg) = self.pkg() {
            // Populate info tab
            let history = imp.pkg_history.borrow();

            let count_label = if history.len() > 1 {
                &format!("   \u{2022}   {}/{}", history.selected() + 1, history.len())
            } else {
                ""
            };

            imp.info_tab.update(&pkg, count_label);

            // Remove delay timer if present
            if let Some(delay_id) = imp.update_delay_id.take() {
                delay_id.remove();

                // Clear files tab
                if imp.files_tab.state() != TabState::Loading {
                    imp.files_tab.pause_view();
                }
            }

            // Start delay timer
            let delay_id = glib::timeout_add_local_once(
                Duration::from_millis(50),
                clone!(
                    #[weak] imp,
                    move || {
                        // Populate files tab
                        imp.files_tab.update_view(&pkg);

                        imp.update_delay_id.take();
                    }
                )
            );

            imp.update_delay_id.replace(Some(delay_id));
        }
    }
}

impl Default for InfoPane {
    //---------------------------------------
    // Default constructor
    //---------------------------------------
    fn default() -> Self {
        glib::Object::builder().build()
    }
}
