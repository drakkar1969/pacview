use std::cell::Cell;
use std::collections::HashSet;
use std::fmt::Write as _;

use gtk::{glib, gio, gdk};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::clone;

use crate::window::PKGS;
use crate::groups_object::GroupsObject;
use crate::enum_traits::EnumExt;

//------------------------------------------------------------------------------
// ENUM: GroupsSearchMode
//------------------------------------------------------------------------------
#[derive(Default, Debug, Eq, PartialEq, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "GroupsSearchMode")]
pub enum GroupsSearchMode {
    #[default]
    All,
    Groups,
    Packages,
}

impl EnumExt for GroupsSearchMode {}

//------------------------------------------------------------------------------
// MODULE: GroupsWindow
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::GroupsWindow)]
    #[template(resource = "/com/github/PacView/ui/groups_window.ui")]
    pub struct GroupsWindow {
        #[template_child]
        pub(super) header_sub_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) search_entry: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub(super) installed_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub(super) copy_button: TemplateChild<gtk::Button>,

        #[template_child]
        pub(super) view: TemplateChild<gtk::ColumnView>,
        #[template_child]
        pub(super) model: TemplateChild<gio::ListStore>,
        #[template_child]
        pub(super) filter_model: TemplateChild<gtk::FilterListModel>,
        #[template_child]
        pub(super) section_sort_model: TemplateChild<gtk::SortListModel>,
        #[template_child]
        pub(super) selection: TemplateChild<gtk::SingleSelection>,
        #[template_child]
        pub(super) search_filter: TemplateChild<gtk::CustomFilter>,
        #[template_child]
        pub(super) installed_filter: TemplateChild<gtk::CustomFilter>,

        #[template_child]
        pub(super) empty_status: TemplateChild<adw::StatusPage>,

        #[property(get, set, builder(GroupsSearchMode::default()))]
        search_mode: Cell<GroupsSearchMode>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for GroupsWindow {
        const NAME: &'static str = "GroupsWindow";
        type Type = super::GroupsWindow;
        type ParentType = adw::Window;

        fn class_init(klass: &mut Self::Class) {
            GroupsObject::ensure_type();

            klass.bind_template();

            // Find key binding
            klass.add_binding(gdk::Key::F, gdk::ModifierType::CONTROL_MASK, |window| {
                let imp = window.imp();

                if !imp.search_entry.has_focus() {
                    imp.search_entry.grab_focus();
                }

                glib::Propagation::Stop
            });

            // Installed key binding
            klass.add_binding(gdk::Key::I, gdk::ModifierType::CONTROL_MASK, |window| {
                let imp = window.imp();

                imp.installed_button.set_active(!imp.installed_button.is_active());

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

    #[glib::derived_properties]
    impl ObjectImpl for GroupsWindow {
        //---------------------------------------
        // Constructor
        //---------------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_widgets();
            obj.setup_controllers();
            obj.setup_actions();
            obj.setup_signals();
        }
    }

    impl WidgetImpl for GroupsWindow {}
    impl WindowImpl for GroupsWindow {}
    impl AdwWindowImpl for GroupsWindow {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: GroupsWindow
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct GroupsWindow(ObjectSubclass<imp::GroupsWindow>)
        @extends adw::Window, gtk::Window, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl GroupsWindow {
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

        // Set search filter function
        imp.search_filter.set_filter_func(clone!(
            #[weak(rename_to = window)] self,
            #[upgrade_or] false,
            move |item| {
                let obj = item
                    .downcast_ref::<GroupsObject>()
                    .expect("Failed to downcast to 'GroupsObject'");

                let search_term = window.imp().search_entry.text().to_lowercase();

                if search_term.is_empty() {
                    true
                } else {
                    match window.search_mode() {
                        GroupsSearchMode::All => {
                            obj.package().to_lowercase().contains(&search_term) ||
                                obj.groups().to_lowercase().contains(&search_term)
                        },
                        GroupsSearchMode::Groups => {
                            obj.groups().to_lowercase().contains(&search_term)
                        },
                        GroupsSearchMode::Packages => {
                            obj.package().to_lowercase().contains(&search_term)
                        },
                    }
                }
            }
        ));

        // Set installed filter function
        imp.installed_filter.set_filter_func(clone!(
            #[weak] imp,
            #[upgrade_or] false,
            move |item| {
                if imp.installed_button.is_active() {
                    let status = item
                        .downcast_ref::<GroupsObject>()
                        .expect("Failed to downcast to 'GroupsObject'")
                        .status();

                    status != "not installed"
                } else {
                    true
                }
            }
        ));

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
    // Setup actions
    //---------------------------------------
    fn setup_actions(&self) {
        let search_action = gio::PropertyAction::new("set-mode", self, "search-mode");

        let search_group = gio::SimpleActionGroup::new();

        self.insert_action_group("search", Some(&search_group));

        search_group.add_action(&search_action);
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
            move |_| {
                imp.search_filter.changed(gtk::FilterChange::Different);
            }
        ));

        // Search mode property notify signal
        self.connect_search_mode_notify(|window| {
            let imp = window.imp();

            let search_mode = window.search_mode();

            if search_mode == GroupsSearchMode::All {
                imp.search_entry.set_placeholder_text(Some("Search all"));
            } else {
                imp.search_entry.set_placeholder_text(Some(&format!("Search for {}", search_mode.nick())));
            }

            imp.search_filter.changed(gtk::FilterChange::Different);
        });

        // Installed button toggled signal
        imp.installed_button.connect_toggled(clone!(
            #[weak] imp,
            move |_| {
                imp.installed_filter.changed(gtk::FilterChange::Different);
            }
        ));

        // Copy button clicked signal
        imp.copy_button.connect_clicked(clone!(
            #[weak(rename_to = window)] self,
            #[weak] imp,
            move |_| {
                let mut groups = String::new();
                let mut body = String::new();

                for item in imp.selection.iter::<glib::Object>().flatten() {
                    let pkg = item
                        .downcast::<GroupsObject>()
                        .expect("Failed to downcast to 'GroupsObject'");

                    let pkg_groups = pkg.groups();

                    if pkg_groups != groups {
                        writeln!(body, "|**{pkg_groups}**||").unwrap();

                        groups = pkg_groups;
                    }

                    writeln!(body, "|{package}|{status}|",
                        package=pkg.package(),
                        status=pkg.status()
                    )
                    .unwrap();
                }

                window.clipboard().set_text(
                    &format!("## Pacman Groups\n|Package Name|Status|\n|---|---|\n{body}")
                );
            }
        ));

        // Selection items changed signal
        imp.selection.connect_items_changed(clone!(
            #[weak] imp,
            move |selection, _, _, _| {
                let n_items = selection.n_items();

                let n_sections = selection.iter::<glib::Object>().flatten()
                    .map(|item|
                        item
                            .downcast::<GroupsObject>()
                            .expect("Failed to downcast to 'GroupsObject'")
                            .groups()
                    )
                    .collect::<HashSet<String>>()
                    .len();

                imp.empty_status.set_visible(n_items == 0);

                imp.header_sub_label.set_label(&format!("{n_items} packages in {n_sections} group{}", if n_sections == 1 { "" } else { "s" }));

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
    pub fn show(&self) {
        let imp = self.imp();

        self.present();

        // Populate if necessary
        if imp.model.n_items() == 0 {
            PKGS.with_borrow(|pkgs| {
                // Get list of packages with groups
                let pkg_list: Vec<GroupsObject> = pkgs.iter()
                    .filter(|pkg| !pkg.groups().is_empty())
                    .flat_map(|pkg|
                        pkg.groups().split(" | ")
                            .map(|group|
                                GroupsObject::new(&pkg.name(), &pkg.status(), &pkg.status_icon_symbolic(), group)
                            )
                            .collect::<Vec<GroupsObject>>()
                    )
                    .collect();

                // Populate column view
                imp.model.splice(0, 0, &pkg_list);

            });
        }
    }
}
