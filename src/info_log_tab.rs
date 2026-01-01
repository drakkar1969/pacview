use std::cell::RefCell;
use std::fmt::Write as _;

use gtk::subclass::prelude::*;
use gtk::prelude::*;
use gtk::{glib, gio};
use glib::clone;

use crate::pkg_object::PkgObject;

//------------------------------------------------------------------------------
// MODULE: InfoLogTab
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::InfoLogTab)]
    #[template(resource = "/com/github/PacView/ui/info_log_tab.ui")]
    pub struct InfoLogTab {
        #[template_child]
        pub(super) header_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) copy_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) model: TemplateChild<gio::ListStore>,
        #[template_child]
        pub(super) selection: TemplateChild<gtk::NoSelection>,
        #[template_child]
        pub(super) paused_status: TemplateChild<adw::StatusPage>,

        #[property(get, set)]
        pkg_name: RefCell<String>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for InfoLogTab {
        const NAME: &'static str = "InfoLogTab";
        type Type = super::InfoLogTab;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for InfoLogTab {
        //---------------------------------------
        // Constructor
        //---------------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_signals();
        }
    }
    impl WidgetImpl for InfoLogTab {}
    impl BoxImpl for InfoLogTab {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: InfoLogTab
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct InfoLogTab(ObjectSubclass<imp::InfoLogTab>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl InfoLogTab {
    //---------------------------------------
    // Setup signals
    //---------------------------------------
    fn setup_signals(&self) {
        let imp = self.imp();

        // Copy button clicked signal
        imp.copy_button.connect_clicked(clone!(
            #[weak(rename_to = tab)] self,
            move |_| {
                let mut output = String::new();

                let _ = writeln!(output, "## {}\n|Log Messages|\n|---|", tab.pkg_name());

                for obj in tab.imp().model.iter::<gtk::StringObject>()
                    .flatten() {
                        let _ = writeln!(output, "{}", obj.string());
                    }

                tab.clipboard().set_text(&output);
            }
        ));

        // Selection items changed signal
        imp.selection.connect_items_changed(clone!(
            #[weak] imp,
            move |selection, _, _, _| {
                let n_items = selection.n_items();

                imp.copy_button.set_sensitive(n_items > 0);
            }
        ));
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
        glib::spawn_future_local(clone!(
            #[weak] imp,
            #[weak] pkg,
            async move {
                let log_lines: Vec<gtk::StringObject> = pkg.log_future().await.iter()
                    .map(|line| gtk::StringObject::new(line))
                    .collect();

                imp.model.splice(0, imp.model.n_items(), &log_lines);
            }
        ));

        self.set_pkg_name(pkg.name());
    }
}
