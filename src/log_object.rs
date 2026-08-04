use std::cell::RefCell;

use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::prelude::ObjectExt;

//------------------------------------------------------------------------------
// STRUCT: LogLine
//------------------------------------------------------------------------------
pub struct LogLine {
    pub date: String,
    pub time: String,
    pub category: String,
    pub message: String
}

impl LogLine {
    //---------------------------------------
    // Parse function
    //---------------------------------------
    pub fn parse(line: &str) -> Option<Self> {
        // Extract timestamp
        let (timestamp, rest) = line.strip_prefix('[')?
            .split_once("] ")?;

        // Extract date and time from timestamp
        let (date, time_with_tz) = timestamp.split_once('T')?;
        let (time, _) = time_with_tz.split_once('+')?;

        // Extract category and message
        let (category, message) = rest.strip_prefix('[')?
            .split_once("] ")?;

        Some(Self {
            date: date.to_owned(),
            time: time.to_owned(),
            category: category.to_owned(),
            message: message.to_owned()
        })
    }
}

//------------------------------------------------------------------------------
// MODULE: LogObject
//------------------------------------------------------------------------------
mod imp {
    use super::*;

    //---------------------------------------
    // Private structure
    //---------------------------------------
    #[derive(Default, glib::Properties)]
    #[properties(wrapper_type = super::LogObject)]
    pub struct LogObject {
        #[property(get, set, construct_only)]
        date: RefCell<String>,
        #[property(get, set, construct_only)]
        time: RefCell<String>,
        #[property(get, set, construct_only)]
        category: RefCell<String>,
        #[property(get, set, construct_only)]
        message: RefCell<String>,
    }

    //---------------------------------------
    // Subclass
    //---------------------------------------
    #[glib::object_subclass]
    impl ObjectSubclass for LogObject {
        const NAME: &'static str = "LogObject";
        type Type = super::LogObject;
    }

    #[glib::derived_properties]
    impl ObjectImpl for LogObject {}
}

//------------------------------------------------------------------------------
// IMPLEMENTATION: LogObject
//------------------------------------------------------------------------------
glib::wrapper! {
    pub struct LogObject(ObjectSubclass<imp::LogObject>);
}

impl LogObject {
    //---------------------------------------
    // New function
    //---------------------------------------
    pub fn new(line: &LogLine) -> Self {
        // Build LogObject
        glib::Object::builder()
            .property("date", &line.date)
            .property("time", &line.time)
            .property("category", &line.category)
            .property("message", &line.message)
            .build()
    }
}
