use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::fmt::Write as _;

use gtk::{glib, gio, gdk};
use adw::subclass::prelude::*;
use adw::prelude::*;
use glib::{clone, Propagation};
use gio::GioFuture;
use gdk::{Key, ModifierType};

use strum::{EnumIter, IntoEnumIterator, AsRefStr};

use crate::{
    groups_object::GroupsObject,
    pkg_object::PkgObject
};

//------------------------------------------------------------------------------
// ENUM: GroupsSearchMode
//------------------------------------------------------------------------------
#[derive(Default, Debug, Eq, PartialEq, Clone, Copy, glib::Enum, EnumIter, AsRefStr)]
#[repr(u32)]
#[enum_type(name = "GroupsSearchMode")]
pub enum GroupsSearchMode {
    #[default]
    All,
    Groups,
    Packages,
}

//------------------------------------------------------------------------------
// MODULE: GroupsDialog
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::GroupsDialog)]
    #[template(resource = "/com/github/PacView/ui/groups_dialog.ui")]
    pub struct GroupsDialog {
        #[template_child]
        pub(super) search_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub(super) search_bar: TemplateChild<gtk::SearchBar>,
        #[template_child]
        pub(super) search_entry: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub(super) search_mode_label: TemplateChild<gtk::Label>,

        #[template_child]
        pub(super) stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) view: TemplateChild<gtk::ListView>,
        #[template_child]
        pub(super) model: TemplateChild<gio::ListStore>,
        #[template_child]
        pub(super) section_sort_model: TemplateChild<gtk::SortListModel>,
        #[template_child]
        pub(super) selection: TemplateChild<gtk::SingleSelection>,
        #[template_child]
        pub(super) search_filter: TemplateChild<gtk::CustomFilter>,
        #[template_child]
        pub(super) installed_filter: TemplateChild<gtk::CustomFilter>,

        #[template_child]
        pub(super) count_label: TemplateChild<gtk::Label>,

        #[template_child]
        pub(super) loading_status: TemplateChild<adw::StatusPage>,

        #[property(get, set)]
        is_loaded: Cell<bool>,
        #[property(get, set, builder(GroupsSearchMode::default()))]
        search_mode: Cell<GroupsSearchMode>,
        #[property(get, set)]
        installed_only: Cell<bool>,

        pub(super) search_term: RefCell<String>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for GroupsDialog {
        const NAME: &'static str = "GroupsDialog";
        type Type = super::GroupsDialog;
        type ParentType = adw::Dialog;

        fn class_init(klass: &mut Self::Class) {
            GroupsObject::ensure_type();

            klass.bind_template();

            // Install actions
            Self::install_actions(klass);

            // Add key bindings
            Self::bind_shortcuts(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for GroupsDialog {
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

    impl WidgetImpl for GroupsDialog {}
    impl AdwDialogImpl for GroupsDialog {}

    impl GroupsDialog {
        //---------------------------------------
        // Install actions
        //---------------------------------------
        fn install_actions(klass: &mut <Self as ObjectSubclass>::Class) {
            // Search mode property action
            klass.install_property_action("search.set-mode", "search-mode");

            // Cycle search mode action
            klass.install_action("search.cycle-mode", None, |dialog, _, _| {
                let new_mode = GroupsSearchMode::iter().cycle()
                    .skip_while(|&mode| mode != dialog.search_mode())
                    .nth(1)
                    .expect("Failed to get 'GroupsSearchMode'");

                dialog.set_search_mode(new_mode);
            });

            // Reverse cycle search mode action
            klass.install_action("search.reverse-cycle-mode", None, |dialog, _, _| {
                let new_mode = GroupsSearchMode::iter().rev().cycle()
                    .skip_while(|&mode| mode != dialog.search_mode())
                    .nth(1)
                    .expect("Failed to get 'GroupsSearchMode'");

                dialog.set_search_mode(new_mode);
            });

            // Installed only property action
            klass.install_property_action("groups.installed-only", "installed-only");

            // Copy action
            klass.install_action("groups.copy", None, |dialog, _, _| {
                let mut groups = String::new();
                let mut output = String::from("## Pacman Groups\n|Package Name|Status|\n|---|---|\n");

                for pkg in dialog.imp().selection.iter::<glib::Object>()
                    .filter_map(|item| item.ok().and_downcast::<GroupsObject>()) {
                        let pkg_groups = pkg.groups();

                        if pkg_groups != groups {
                            writeln!(output, "|**{pkg_groups}**||").unwrap();

                            groups = pkg_groups;
                        }

                        writeln!(output, "|{package}|{status}|",
                            package=pkg.package(),
                            status=pkg.status()
                        )
                        .unwrap();
                    }

                dialog.clipboard().set_text(&output);
            });
        }

        //---------------------------------------
        // Bind shortcuts
        //---------------------------------------
        fn bind_shortcuts(klass: &mut <Self as ObjectSubclass>::Class) {
            // Find key binding
            klass.add_binding(Key::F, ModifierType::CONTROL_MASK, |dialog| {
                dialog.imp().search_bar.set_search_mode(true);

                Propagation::Stop
            });

            // Cycle search mode key bindings
            klass.add_binding_action(Key::M, ModifierType::CONTROL_MASK, "search.cycle-mode");
            klass.add_binding_action(Key::M, ModifierType::CONTROL_MASK | ModifierType::SHIFT_MASK, "search.reverse-cycle-mode");

            // Installed key binding
            klass.add_binding_action(Key::I, ModifierType::CONTROL_MASK, "groups.installed-only");

            // Copy key binding
            klass.add_binding_action(Key::C, ModifierType::CONTROL_MASK | ModifierType::SHIFT_MASK, "groups.copy");
        }
    }
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: GroupsDialog
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct GroupsDialog(ObjectSubclass<imp::GroupsDialog>)
        @extends adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::ShortcutManager;
}

impl GroupsDialog {
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
                imp.search_term.replace(entry.text().trim().to_lowercase());

                imp.search_filter.changed(gtk::FilterChange::Different);
            }
        ));

        // Search mode property notify signal
        self.connect_search_mode_notify(|dialog| {
            let imp = dialog.imp();

            imp.search_mode_label.set_label(dialog.search_mode().as_ref());

            imp.search_filter.changed(gtk::FilterChange::Different);
        });

        // Installed only property notify signal
        self.connect_installed_only_notify(|dialog| {
            dialog.imp().installed_filter.changed(gtk::FilterChange::Different);
        });

        // Section sort model items changed signal
        imp.section_sort_model.connect_items_changed(clone!(
            #[weak(rename_to = dialog)] self,
            move |sort_model, _, _, _| {
                let imp = dialog.imp();

                let n_items = sort_model.n_items();
                let mut n_sections = 0;

                if n_items != 0 {
                    let mut index = 0;

                    while index < n_items {
                        let (_, end) = sort_model.section(index);

                        n_sections += 1;
                        index = end;
                    }
                }

                let set: HashSet<String> = sort_model.iter::<glib::Object>()
                    .flatten()
                    .filter_map(|obj| obj.downcast::<GroupsObject>().map(|group| group.package()).ok())
                    .collect();

                let n_unique = set.len();

                imp.stack.set_visible_child_name(
                    if n_items == 0 { "empty" } else { "view" }
                );

                imp.count_label.set_label(&format!(
                    "{n_unique} unique package{} in {n_sections} group{}",
                    if n_unique == 1 { "" } else { "s" },
                    if n_sections == 1 { "" } else { "s" }
                ));

                dialog.action_set_enabled("groups.copy", n_items > 0);
                dialog.action_set_enabled("groups.installed-only", n_items > 0);
            }
        ));
    }

    //---------------------------------------
    // Setup widgets
    //---------------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Set search bar key capture widget and connect entry
        imp.search_bar.set_key_capture_widget(Some(&imp.view.get()));
        imp.search_bar.connect_entry(&imp.search_entry.get());

        // Bind search button state to search bar visibility
        imp.search_button.bind_property("active", &imp.search_bar.get(), "search-mode-enabled")
            .bidirectional()
            .sync_create()
            .build();

        // Set search filter function
        imp.search_filter.set_filter_func(clone!(
            #[weak(rename_to = dialog)] self,
            #[upgrade_or] false,
            move |item| {
                let search_term = dialog.imp().search_term.borrow();

                if search_term.is_empty() {
                    return true;
                }

                let obj = item
                    .downcast_ref::<GroupsObject>()
                    .expect("Failed to downcast to 'GroupsObject'");

                let is_match = |prop: &str| -> bool {
                    prop.as_bytes()
                        .windows(search_term.len())
                        .any(|window| window.eq_ignore_ascii_case(search_term.as_bytes()))
                };

                match dialog.search_mode() {
                    GroupsSearchMode::All => {
                        is_match(&obj.package()) || is_match(&obj.groups())
                    }
                    GroupsSearchMode::Groups => {
                        is_match(&obj.groups())
                    }
                    GroupsSearchMode::Packages => {
                        is_match(&obj.package())
                    }
                }
            }
        ));

        // Set installed filter function
        imp.installed_filter.set_filter_func(clone!(
            #[weak(rename_to = dialog)] self,
            #[upgrade_or] false,
            move |item| {
                if dialog.installed_only() {
                    let status = item
                        .downcast_ref::<GroupsObject>()
                        .expect("Failed to downcast to 'GroupsObject'")
                        .status();

                    !status.is_empty()
                } else {
                    true
                }
            }
        ));

        // Set initial focus on view
        imp.view.grab_focus();
    }

    //---------------------------------------
    // Populate dialog
    //---------------------------------------
    fn populate(&self, pkg_model: &gio::ListStore) {
        glib::spawn_future_local(clone!(
            #[weak(rename_to = dialog)] self,
            #[weak] pkg_model,
            async move {
                let imp = dialog.imp();

                imp.loading_status.set_visible(true);

                // Spawn future to get packages with groups
                let packages: Vec<GroupsObject> = GioFuture::new(&pkg_model, |model, _, result| {
                    let packages = model.iter::<PkgObject>()
                        .flatten()
                        .flat_map(|pkg| {
                            pkg.groups().iter()
                                .map(|group| {
                                    GroupsObject::new(&pkg.name(), pkg.status(), pkg.status_tag_type(), group)
                                })
                                .collect::<Vec<GroupsObject>>()
                        })
                        .collect();

                    result.resolve(packages);
                })
                .await;

                imp.model.splice(0, imp.model.n_items(), &packages);

                imp.loading_status.set_visible(false);

                dialog.set_is_loaded(true);
            }
        ));
    }

    //---------------------------------------
    // Present dialog
    //---------------------------------------
    pub fn present(&self, parent: Option<&impl IsA<gtk::Widget>>, pkg_model: &gio::ListStore) {
        adw::Dialog::present(self.upcast_ref::<adw::Dialog>(), parent);

        if !self.is_loaded() {
            self.populate(pkg_model);
        }
    }
}

impl Default for GroupsDialog {
    //---------------------------------------
    // Default constructor
    //---------------------------------------
    fn default() -> Self {
        glib::Object::builder().build()
    }
}
