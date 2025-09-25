use std::marker::PhantomData;
use std::sync::OnceLock;

use gtk::{gdk, glib};
use adw::subclass::prelude::*;
use gtk::prelude::*;
use glib::clone;
use glib::subclass::Signal;

//------------------------------------------------------------------------------
// MODULE: SearchTag
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, gtk::CompositeTemplate, glib::Properties)]
    #[properties(wrapper_type = super::SearchTag)]
    #[template(resource = "/com/github/PacView/ui/search_tag.ui")]
    pub struct SearchTag {
        #[template_child]
        pub(super) label: TemplateChild<gtk::Label>,

        #[property(set = Self::set_text)]
        text: PhantomData<String>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for SearchTag {
        const NAME: &'static str = "SearchTag";
        type Type = super::SearchTag;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.set_css_name("searchtag");
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for SearchTag {
        //---------------------------------------
        // Signals
        //---------------------------------------
        fn signals() -> &'static [Signal] {
            static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| {
                vec![
                    Signal::builder("clicked")
                        .param_types([bool::static_type()])
                        .build(),
                ]
            })
        }

        //---------------------------------------
        // Constructor
        //---------------------------------------
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            obj.setup_controllers();
        }
    }

    impl WidgetImpl for SearchTag {}
    impl BinImpl for SearchTag {}

    impl SearchTag {
        fn set_text(&self, text: &str) {
            self.label.set_label(text);
        }
    }
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: SearchTag
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct SearchTag(ObjectSubclass<imp::SearchTag>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl SearchTag {
    //---------------------------------------
    // Setup controllers
    //---------------------------------------
    fn setup_controllers(&self) {
        // Click controller
        let gesture_click = gtk::GestureClick::new();
        gesture_click.set_button(gdk::BUTTON_PRIMARY);

        gesture_click.connect_released(clone!(
            #[weak(rename_to = tag)] self,
            move |gesture_click, _, _, _| {
                let shift = gesture_click.current_event_state() == (gdk::ModifierType::BUTTON1_MASK | gdk::ModifierType::SHIFT_MASK);

                tag.emit_by_name::<()>("clicked", &[&shift]);

                gesture_click.set_state(gtk::EventSequenceState::Claimed);
            }
        ));

        self.add_controller(gesture_click);
    }
}
