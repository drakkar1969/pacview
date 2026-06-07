use std::cell::{RefCell, OnceCell};
use std::marker::PhantomData;
use std::time::Duration;

use gtk::glib;
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::{clone, closure_local};

use crate::{
    package_view::PackageView,
    info_details_tab::InfoDetailsTab,
    info_files_tab::InfoFilesTab,
    info_log_tab::InfoLogTab,
    history_list::HistoryList,
    pkg_object::PkgObject,
    text_widget::TextWidget
};

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
        pub(super) prev_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) next_button: TemplateChild<gtk::Button>,

        #[template_child]
        pub(super) main_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(super) tab_switcher: TemplateChild<adw::InlineViewSwitcher>,
        #[template_child]
        pub(super) tab_stack: TemplateChild<adw::ViewStack>,

        #[template_child]
        pub(super) info_tab: TemplateChild<InfoDetailsTab>,
        #[template_child]
        pub(super) files_tab: TemplateChild<InfoFilesTab>,
        #[template_child]
        pub(super) log_tab: TemplateChild<InfoLogTab>,

        #[property(get = Self::pkg, set = Self::set_pkg, nullable)]
        pkg: PhantomData<Option<PkgObject>>,

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
        // Property getter/setter
        //---------------------------------------
        fn pkg(&self) -> Option<PkgObject> {
            self.pkg_history.borrow().selected_item()
        }

        fn set_pkg(&self, pkg: Option<PkgObject>) {
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
    // InfoRow pkg link handler
    //---------------------------------------
    fn pkg_link_handler(&self, pkg_name: &str, pkg_version: &str) {
        // Find link package in pacman databases or AUR search results
        let pkg_link = pkg_name.to_owned() + pkg_version;

        let pkg_model = self.package_view().pkg_model();
        let aur_model = self.package_view().aur_model();

        let new_pkg = PkgObject::find_satisfier(&pkg_link, &pkg_model)
            .or_else(|| {
                aur_model.iter::<PkgObject>()
                    .flatten()
                    .find(|pkg| pkg.name() == pkg_name)
                    .or_else(|| {
                        aur_model.iter::<PkgObject>()
                            .flatten()
                            .find(|pkg| pkg.provides().iter().any(|s| s == &pkg_link))
                    })
            });

        // If link package found
        if let Some(pkg) = new_pkg {
            let history = self.imp().pkg_history.borrow();

            // If link package is in history, select it
            // Otherwise append it after selected history package
            history.select_or_append(pkg);

            // Display link package
            self.update_display();
        }
    }

    //---------------------------------------
    // Setup signals
    //---------------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // Pkg property notify signal
        self.connect_pkg_notify(|pane| {
            let imp = pane.imp();

            let pkg_is_some = pane.pkg().is_some();

            imp.main_stack.set_visible_child_name(
                if pkg_is_some { "properties" } else { "empty" }
            );

            imp.tab_switcher.set_sensitive(pkg_is_some);
        });

        // Previous button clicked signal
        imp.prev_button.connect_clicked(clone!(
            #[weak(rename_to = pane)] self,
            move |_| {
                pane.display_prev();
            }
        ));

        // Next button clicked signal
        imp.next_button.connect_clicked(clone!(
            #[weak(rename_to = pane)] self,
            move |_| {
                pane.display_next();
            }
        ));
    }

    //---------------------------------------
    // Setup widgets
    //---------------------------------------
    fn setup_widgets(&self) {
        let imp = self.imp();

        // Bind history list properties to widgets
        let history = imp.pkg_history.borrow();

        history.bind_property("peek-previous", &imp.prev_button.get(), "sensitive")
            .sync_create()
            .build();

        history.bind_property("peek-next", &imp.next_button.get(), "sensitive")
            .sync_create()
            .build();

        // Setup info pane pkg link handler
        imp.info_tab.setup_details_listbox(closure_local!(
            #[weak(rename_to = pane)] self,
            move |_: TextWidget, pkg_name: &str, pkg_version: &str| {
                pane.pkg_link_handler(pkg_name, pkg_version);
            }
        ));
    }

    //---------------------------------------
    // Public display functions
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

                // Clear files/log tabs
                imp.files_tab.pause_view();
                imp.log_tab.pause_view();
            }

            // Start delay timer
            let delay_id = glib::timeout_add_local_once(
                Duration::from_millis(50),
                clone!(
                    #[weak] imp,
                    move || {
                        // Populate files/log tabs
                        imp.files_tab.update_view(&pkg);
                        imp.log_tab.update_view(&pkg);

                        imp.update_delay_id.take();
                    }
                )
            );

            imp.update_delay_id.replace(Some(delay_id));
        }
    }

    pub fn display_prev(&self) {
        let history = self.imp().pkg_history.borrow();

        if history.peek_previous() {
            history.select_previous();

            self.update_display();
        }
    }

    pub fn display_next(&self) {
        let history = self.imp().pkg_history.borrow();

        if history.peek_next() {
            history.select_next();

            self.update_display();
        }
    }

    //---------------------------------------
    // Other public functions
    //---------------------------------------
    pub fn set_visible_tab(&self, tab: &str) {
        let imp = self.imp();

        if imp.tab_switcher.is_sensitive() {
            imp.tab_stack.set_visible_child_name(tab);
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
