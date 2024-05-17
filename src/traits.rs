use glib::types::StaticType;
use glib::value::ToValue;
use glib::variant::ToVariant;
use glib::{EnumClass, EnumValue, HasParamSpec, ParamSpecEnum, Variant};

//------------------------------------------------------------------------------
// TRAIT: EnumValueExt
//------------------------------------------------------------------------------
pub trait EnumValueExt
where Self: ToValue + StaticType + HasParamSpec<ParamSpec = ParamSpecEnum> + Sized {
    fn enum_value(&self) -> EnumValue {
        EnumValue::from_value(&self.to_value())
            .map(|(_, enum_value)| *enum_value)
            .expect("Could not get 'EnumValue'")
    }

    fn name(&self) -> String {
        self.enum_value()
            .name()
            .to_string()
    }

    fn nick(&self) -> String {
        self.enum_value()
            .nick()
            .to_string()
    }

    fn to_variant(&self) -> Variant {
        self.enum_value()
            .nick()
            .to_variant()
    }
}

//------------------------------------------------------------------------------
// TRAIT: EnumClassExt
//------------------------------------------------------------------------------
pub trait EnumClassExt
where Self: StaticType + HasParamSpec<ParamSpec = ParamSpecEnum> + Sized {
    fn enum_class() -> EnumClass {
        EnumClass::new::<Self>()
    }

    fn previous_nick(nick: &str) -> String {
        let enum_class = Self::enum_class();

        let mut iter = enum_class.values().iter()
            .rev()
            .cycle();

        iter.find(|value| value.nick() == nick);

        iter.next()
            .expect("Could not get 'EnumValue'")
            .nick()
            .to_string()
    }

    fn next_nick(nick: &str) -> String {
        let enum_class = Self::enum_class();

        let mut iter = enum_class.values().iter()
            .cycle();

        iter.find(|value| value.nick() == nick);

        iter.next()
            .expect("Could not get 'EnumValue'")
            .nick()
            .to_string()
    }
}
