use std::cell::RefCell;
use std::collections::HashSet;

use gtk::{glib, gio, gdk};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::clone;

use itertools::Itertools;

use crate::groups_object::GroupsObject;
use crate::pkg_object::PkgObject;

//------------------------------------------------------------------------------
// MODULE: GroupsWindow
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/com/github/PacView/ui/groups_window.ui")]
    pub struct GroupsWindow {
        #[template_child]
        pub(super) header_sub_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) search_entry: TemplateChild<gtk::SearchEntry>,
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
        pub(super) search_filter: TemplateChild<gtk::StringFilter>,

        pub(super) bindings: RefCell<Vec<glib::Binding>>,
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

            klass.add_binding_action(gdk::Key::Escape, gdk::ModifierType::NO_MODIFIER_MASK, "window.close");

            // Add find key binding
            klass.add_binding(gdk::Key::F, gdk::ModifierType::CONTROL_MASK, |window| {
                let imp = window.imp();

                if !imp.search_entry.has_focus() {
                    imp.search_entry.grab_focus();
                }

                glib::Propagation::Stop
            });

            // Add copy key binding
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

    impl ObjectImpl for GroupsWindow {
        //---------------------------------------
        // Constructor
        //---------------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_widgets();
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

        // Set initial focus on view
        imp.view.grab_focus();
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

        // Copy button clicked signal
        imp.copy_button.connect_clicked(clone!(
            #[weak(rename_to = window)] self,
            #[weak] imp,
            move |_| {
                let mut group = String::from("");

                let copy_text = format!("## Pacman Groups\n|Package Name|Status|\n|---|---|\n{body}",
                    body=imp.selection.iter::<glib::Object>().flatten()
                        .map(|item| {
                            let pkg = item
                                .downcast::<GroupsObject>()
                                .expect("Could not downcast to 'GroupsObject'");

                            let mut line = String::from("");

                            let pkg_group = pkg.groups();

                            if pkg_group != group {
                                line.push_str(&format!("|**{group}**||\n",
                                    group=pkg_group
                                ));

                                group = pkg_group;
                            }

                            line.push_str(&format!("|{package}|{status}|",
                                package=pkg.name(),
                                status=pkg.status()
                            ));

                            line
                        })
                        .join("\n")
                );

                window.clipboard().set_text(&copy_text);
            }
        ));
    }

    //---------------------------------------
    // Clear window
    //---------------------------------------
    pub fn clear(&self) {
        for binding in self.imp().bindings.take() {
            binding.unbind();
        }

        self.imp().model.remove_all();
    }

    //---------------------------------------
    // Show window
    //---------------------------------------
    pub fn show(&self, pkgs: &[PkgObject]) {
        let imp = self.imp();

        self.present();

        // Populate if necessary
        if imp.model.n_items() == 0 {
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

            // Bind view count to header sub label
            let label_binding = imp.selection.bind_property("n-items", &imp.header_sub_label.get(), "label")
                .transform_to(move |binding, n_items: u32| {
                    let selection = binding.source()
                        .and_downcast::<gtk::SingleSelection>()
                        .expect("Could not downcast to 'FilterListModel'");

                    let section_map: HashSet<String> = selection.iter::<glib::Object>().flatten()
                        .map(|item| {
                            item
                                .downcast::<GroupsObject>()
                                .expect("Could not downcast to 'GroupsObject'")
                                .groups()
                        })
                        .collect();

                    let section_len = section_map.len();

                    Some(format!("{n_items} packages in {section_len} group{}",
                        if section_len != 1 {"s"} else {""}
                    ))
                })
                .sync_create()
                .build();

            // Bind view count to copy button state
            let copy_binding = imp.selection.bind_property("n-items", &imp.copy_button.get(), "sensitive")
                .transform_to(|_, n_items: u32|Some(n_items > 0))
                .sync_create()
                .build();

            imp.bindings.replace(vec![label_binding, copy_binding]);
        }
    }
}
