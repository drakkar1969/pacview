use std::process::Command;

use gtk::{glib, pango};
use gtk::prelude::ToValue;

//------------------------------------------------------------------------------
// MODULE: Utils
//------------------------------------------------------------------------------
pub struct Utils;

//------------------------------------------------------------------------------
// IMPLEMENTATION: Utils
//------------------------------------------------------------------------------
impl Utils {
    //-----------------------------------
    // Run command function
    //-----------------------------------
    pub fn run_command(cmd: &str) -> (Option<i32>, String) {
        let mut code: Option<i32> = None;
        let mut stdout: String = String::from("");

        if let Some(params) = shlex::split(cmd) {
            if !params.is_empty() {
                if let Ok(output) = Command::new(&params[0]).args(&params[1..]).output() {
                    code = output.status.code();
                    stdout = String::from_utf8(output.stdout).unwrap_or_default();
                }
            }
        }

        (code, stdout)
    }

    //-----------------------------------
    // Size to string function
    //-----------------------------------
    pub fn size_to_string(size: i64, decimals: usize) -> String {
        let mut size = size as f64;

        if size == 0.0 {
            String::from("0 B")
        } else {
            let mut unit = "";
    
            for u in ["B", "KiB", "MiB", "GiB", "TiB", "PiB"] {
                unit = u;
    
                if size < 1024.0 || u == "PiB" {
                    break;
                }
    
                size /= 1024.0;
            }
    
            format!("{size:.decimals$} {unit}")
        }
    }

    //-----------------------------------
    // Date to string function
    //-----------------------------------
    pub fn date_to_string(date: i64, format: &str) -> String {
        if date == 0 {
            String::from("")
        } else {
            let datetime = glib::DateTime::from_unix_local(date).expect("Datetime from Unix error");

            datetime.format(format).expect("Datetime format error").to_string()
        }
    }

    //-----------------------------------
    // Pango font string to CSS function
    //-----------------------------------
    pub fn pango_font_string_to_css(font_str: &str) -> String {
        let mut css = String::from("");
        
        let font_desc = pango::FontDescription::from_string(font_str);

        let mask = font_desc.set_fields();

        if mask.contains(pango::FontMask::FAMILY) {
            if let Some(family) = font_desc.family() {
                css += &format!("font-family: \"{family}\"; ");
            }
        }

        if mask.contains(pango::FontMask::SIZE) {
            css += &format!("font-size: {}pt; ", font_desc.size()/pango::SCALE);
        }

        if mask.contains(pango::FontMask::WEIGHT) {
            match font_desc.weight() {
                pango::Weight::Normal => css += "font-weight: normal; ",
                pango::Weight::Bold => css += "font-weight: bold; ",
                pango::Weight::Thin => css += "font-weight: 100; ",
                pango::Weight::Ultralight => css += "font-weight: 200; ",
                pango::Weight::Light => css += "font-weight: 300; ",
                pango::Weight::Semilight => css += "font-weight: 300; ",
                pango::Weight::Book => css += "font-weight: 400; ",
                pango::Weight::Medium => css += "font-weight: 500; ",
                pango::Weight::Semibold => css += "font-weight: 600; ",
                pango::Weight::Ultrabold => css += "font-weight: 800; ",
                pango::Weight::Heavy | pango::Weight::Ultraheavy => css += "font-weight: 900; ",
                _ => unreachable!()
            }
        }

        if mask.contains(pango::FontMask::STYLE) {
            if let Some((_, value)) = glib::EnumValue::from_value(&font_desc.style().to_value()) {
                css += &format!("font-style: {}; ", value.nick());
            }
        }

        css
    }
}
