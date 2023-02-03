#!/usr/bin/env python

import gi, sys, os, urllib.parse, subprocess, shlex, re, threading

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Gtk, Adw, Gio, GObject, Pango, Gdk, GLib

import pyalpm

from objects import PkgStatus, PkgObject, PkgProperty, StatsItem

# Global path variable
app_dir = os.path.abspath(os.path.dirname(sys.argv[0]))

# Global gresource file
gresource = Gio.Resource.load(os.path.join(app_dir, "com.github.PacView.gresource"))
gresource._register()

#------------------------------------------------------------------------------
#-- CLASS: STATSWINDOW
#------------------------------------------------------------------------------
@Gtk.Template(resource_path="/com/github/PacView/ui/statswindow.ui")
class StatsWindow(Adw.Window):
	__gtype_name__ = "StatsWindow"

	#-----------------------------------
	# Class widget variables
	#-----------------------------------
	model = Gtk.Template.Child()

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

		# Initialize widgets
		total_count = 0
		total_size = 0

		for db in app.main_window.pacman_db_names:
			pkg_list = [pkg for pkg in app.main_window.pkg_objects if pkg.repository == db and (pkg.status_flags & PkgStatus.INSTALLED)]

			count = len(pkg_list)
			total_count += count

			size = sum([pkg.install_size_raw for pkg in pkg_list])
			total_size += size

			self.model.append(StatsItem(
				db if db.isupper() else str.title(db),
				count,
				PkgObject.size_to_str(size, 2)
			))

		self.model.append(StatsItem(
			"<b>Total</b>",
			f'<b>{total_count}</b>',
			f'<b>{PkgObject.size_to_str(total_size, 2)}</b>'
		))

	#-----------------------------------
	# Key press signal handler
	#-----------------------------------
	@Gtk.Template.Callback()
	def on_key_pressed(self, keyval, keycode, user_data, state):
		if keycode == Gdk.KEY_Escape and state == 0: self.close()

	#-----------------------------------
	# Factory signal handlers
	#-----------------------------------
	@Gtk.Template.Callback()
	def on_setup_left(self, factory, item):
		label = Gtk.Label(xalign=0, use_markup=True)
		item.set_child(label)

	@Gtk.Template.Callback()
	def on_setup_right(self, factory, item):
		label = Gtk.Label(xalign=1, use_markup=True)
		item.set_child(label)

	@Gtk.Template.Callback()
	def on_bind_repository(self, factory, item):
		item.get_child().set_label(item.get_item().repository)

	@Gtk.Template.Callback()
	def on_bind_count(self, factory, item):
		item.get_child().set_label(item.get_item().count)

	@Gtk.Template.Callback()
	def on_bind_size(self, factory, item):
		item.get_child().set_label(item.get_item().size)

#------------------------------------------------------------------------------
#-- CLASS: VTOGGLEBUTTON
#------------------------------------------------------------------------------
@Gtk.Template(resource_path="/com/github/PacView/ui/vtogglebutton.ui")
class VToggleButton(Gtk.ToggleButton):
	__gtype_name__ = "VToggleButton"

	#-----------------------------------
	# Properties
	#-----------------------------------
	str_id = GObject.Property(type=str, default="")

	icon = GObject.Property(type=str, default="")
	text = GObject.Property(type=str, default="")

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

#------------------------------------------------------------------------------
#-- CLASS: PKGDETAILSWINDOW
#------------------------------------------------------------------------------
@Gtk.Template(resource_path="/com/github/PacView/ui/pkgdetailswindow.ui")
class PkgDetailsWindow(Adw.Window):
	__gtype_name__ = "PkgDetailsWindow"

	#-----------------------------------
	# Class widget variables
	#-----------------------------------
	pkg_label = Gtk.Template.Child()

	content_stack = Gtk.Template.Child()

	file_header_label = Gtk.Template.Child()
	files_model = Gtk.Template.Child()

	tree_label = Gtk.Template.Child()

	log_model = Gtk.Template.Child()

	cache_header_label = Gtk.Template.Child()
	cache_model = Gtk.Template.Child()

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, pkg_object, *args, **kwargs):
		super().__init__(*args, **kwargs)

		# Set tree label font
		self.tree_label.set_attributes(Pango.AttrList.from_string('0 -1 font-desc "Source Code Pro 11"'))

		# Initialize widgets
		if pkg_object is not None:
			# Set package name
			self.pkg_label.set_text(f'{pkg_object.repository}/{pkg_object.name}')

			# Populate file list
			self.file_header_label.set_text(f'Files ({len(pkg_object.files_list)})')
			self.files_model.splice(0, 0, pkg_object.files_list)

			# Populate dependency tree
			pkg_tree = subprocess.run(shlex.split(f'pactree{"" if (pkg_object.status_flags & PkgStatus.INSTALLED) else " -s"} {pkg_object.name}'), stdout=subprocess.PIPE, stderr=subprocess.PIPE)

			self.tree_label.set_label(re.sub(" provides.+", "", pkg_tree.stdout.decode()))

			# Populate log
			pkg_log = subprocess.run(shlex.split(f'paclog --no-color --package={pkg_object.name}'), stdout=subprocess.PIPE, stderr=subprocess.PIPE)

			log_lines = [re.sub("\[(.+)T(.+)\+.+\] (.+)", r"\1 \2 : \3", l) for l in pkg_log.stdout.decode().split('\n') if l != ""]

			self.log_model.splice(0, 0, log_lines[::-1]) # Reverse list

			# Populate cache
			pkg_cache = subprocess.run(shlex.split(f'paccache -vdk0 {pkg_object.name}'), stdout=subprocess.PIPE, stderr=subprocess.PIPE)

			cache_lines = [l for l in pkg_cache.stdout.decode().split('\n') if (l != "" and l.startswith("==>") == False and l.endswith(".sig") == False)]

			self.cache_header_label.set_text(f'Cache ({len(cache_lines)})')
			self.cache_model.splice(0, 0, cache_lines)

	#-----------------------------------
	# Key press signal handler
	#-----------------------------------
	@Gtk.Template.Callback()
	def on_key_pressed(self, keyval, keycode, user_data, state):
		if keycode == Gdk.KEY_Escape and state == 0: self.close()

	#-----------------------------------
	# Button signal handler
	#-----------------------------------
	@Gtk.Template.Callback()
	def on_button_toggled(self, button):
		if button.get_active() == True:
			self.content_stack.set_visible_child_name(button.str_id)

#------------------------------------------------------------------------------
#-- CLASS: PKGINFOPANE
#------------------------------------------------------------------------------
@Gtk.Template(resource_path="/com/github/PacView/ui/pkginfopane.ui")
class PkgInfoPane(Gtk.Overlay):
	__gtype_name__ = "PkgInfoPane"

	#-----------------------------------
	# Class widget variables
	#-----------------------------------
	model = Gtk.Template.Child()

	overlay_toolbar = Gtk.Template.Child()
	prev_button = Gtk.Template.Child()
	next_button = Gtk.Template.Child()

	empty_label = Gtk.Template.Child()

	#-----------------------------------
	# Properties
	#-----------------------------------
	__pkg_list = []
	__pkg_index = -1

	@GObject.Property(type=PkgObject, default=None)
	def pkg_object(self):
		return(self.__pkg_list[self.__pkg_index] if (self.__pkg_index >= 0 and self.__pkg_index < len(self.__pkg_list)) else None)

	@pkg_object.setter
	def pkg_object(self, value):
		self.__pkg_list = [value]
		self.__pkg_index = 0

		self.display_package(value)

		self.overlay_toolbar.set_visible(False)

		self.empty_label.set_visible(value is None)

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

	#-----------------------------------
	# Factory signal handlers
	#-----------------------------------
	@Gtk.Template.Callback()
	def on_setup_value(self, factory, item):
		image = Gtk.Image()

		button = Gtk.Button(icon_name="edit-copy")
		button.add_css_class("flat")
		button.add_css_class("inline-button")
		button.set_can_focus(False)
		button.connect("clicked", self.on_copybtn_clicked)

		label = Gtk.Label(halign=Gtk.Align.START, wrap_mode=Pango.WrapMode.WORD, wrap=True, xalign=0, use_markup=True)
		label.connect("activate-link", self.on_link_activated)

		box = Gtk.Box(spacing=6)
		box.append(image)
		box.append(button)
		box.append(label)

		item.set_child(box)

	@Gtk.Template.Callback()
	def on_bind_value(self, factory, item):
		child = item.get_child()
		obj = item.get_item()
		
		image = child.get_first_child()
		button = child.get_first_child().get_next_sibling()
		label = child.get_last_child()

		icon = obj.prop_icon

		image.set_visible(icon != "")
		image.set_from_icon_name(icon)

		label.set_label(obj.prop_value)

		button.set_visible(obj.prop_copy)

	#-----------------------------------
	# Link signal handler
	#-----------------------------------
	def on_link_activated(self, label, url):
		parse_url = urllib.parse.urlsplit(url)

		if parse_url.scheme != "pkg": return(False)

		pkg_name = parse_url.netloc

		pkg_dict = dict([(pkg.name, pkg) for pkg in app.main_window.pkg_objects])

		new_pkg = None

		if pkg_name in pkg_dict.keys():
			new_pkg = pkg_dict[pkg_name]
		else:
			for pkg in pkg_dict.values():
				if [s for s in pkg.provides_list if pkg_name in s] != []:
					new_pkg = pkg
					break

		if new_pkg is not None and new_pkg is not self.__pkg_list[self.__pkg_index]:
			self.__pkg_list = self.__pkg_list[:self.__pkg_index+1]
			self.__pkg_list.append(new_pkg)

			self.__pkg_index += 1

			self.display_package(new_pkg)

			self.overlay_toolbar.set_visible(True)

		return(True)

	#-----------------------------------
	# Copy button signal handler
	#-----------------------------------
	def on_copybtn_clicked(self, button):
		clipboard = self.get_clipboard()

		content = Gdk.ContentProvider.new_for_value(GObject.Value(str, button.get_next_sibling().get_label()))

		clipboard.set_content(content)

	#-----------------------------------
	# Display functions
	#-----------------------------------
	def display_package(self, obj):
		self.prev_button.set_sensitive(self.__pkg_index > 0)
		self.next_button.set_sensitive(self.__pkg_index < len(self.__pkg_list) - 1)

		self.model.remove_all()

		if obj is not None:
			self.model.append(PkgProperty("Name", f'<b>{obj.name}</b>'))
			if obj.update_version != "": self.model.append(PkgProperty("Version", obj.update_version, prop_icon="pkg-update"))
			else: self.model.append(PkgProperty("Version", obj.version))
			self.model.append(PkgProperty("Description", obj.description))
			self.model.append(PkgProperty("URL", obj.url))
			if obj.repository in app.main_window.sync_db_names: self.model.append(PkgProperty("Package URL", obj.package_url))
			if obj.repository == "AUR": self.model.append(PkgProperty("AUR URL", obj.package_url))
			self.model.append(PkgProperty("Licenses", obj.licenses))
			self.model.append(PkgProperty("Status", obj.status if (obj.status_flags & PkgStatus.INSTALLED) else "not installed", prop_icon=obj.status_icon))
			self.model.append(PkgProperty("Repository", obj.repository))
			if obj.group != "":self.model.append(PkgProperty("Groups", obj.group))
			if obj.provides != "None": self.model.append(PkgProperty("Provides", obj.provides))
			self.model.append(PkgProperty("Dependencies", obj.depends))
			if obj.optdepends != "None": self.model.append(PkgProperty("Optional", obj.optdepends))
			self.model.append(PkgProperty("Required By", obj.required_by))
			if obj.optional_for != "None": self.model.append(PkgProperty("Optional For", obj.optional_for))
			if obj.conflicts != "None": self.model.append(PkgProperty("Conflicts With", obj.conflicts))
			if obj.replaces != "None": self.model.append(PkgProperty("Replaces", obj.replaces))
			self.model.append(PkgProperty("Architecture", obj.architecture))
			self.model.append(PkgProperty("Maintainer", obj.maintainer))
			self.model.append(PkgProperty("Build Date", obj.build_date_long))
			if obj.install_date_long != "": self.model.append(PkgProperty("Install Date", obj.install_date_long))
			if obj.download_size != "": self.model.append(PkgProperty("Download Size", obj.download_size))
			self.model.append(PkgProperty("Installed Size", obj.install_size))
			self.model.append(PkgProperty("Install Script", obj.install_script))
			if obj.sha256sum != "": self.model.append(PkgProperty("SHA256 Sum", obj.sha256sum, prop_copy=True))
			if obj.md5sum != "": self.model.append(PkgProperty("MD5 Sum", obj.md5sum, prop_copy=True))

	def display_prev_package(self):
		if self.__pkg_index > 0:
			self.__pkg_index -=1

			self.display_package(self.pkg_object)

	def display_next_package(self):
		if self.__pkg_index < len(self.__pkg_list) - 1:
			self.__pkg_index +=1

			self.display_package(self.pkg_object)

#------------------------------------------------------------------------------
#-- CLASS: PKGCOLUMNVIEW
#------------------------------------------------------------------------------
@Gtk.Template(resource_path="/com/github/PacView/ui/pkgcolumnview.ui")
class PkgColumnView(Gtk.Overlay):
	__gtype_name__ = "PkgColumnView"

	#-----------------------------------
	# Class widget variables
	#-----------------------------------
	view = Gtk.Template.Child()
	selection = Gtk.Template.Child()
	filter_model = Gtk.Template.Child()
	model = Gtk.Template.Child()
	empty_label = Gtk.Template.Child()

	repo_filter = Gtk.Template.Child()
	status_filter = Gtk.Template.Child()
	search_filter = Gtk.Template.Child()

	version_sorter = Gtk.Template.Child()

	package_column = Gtk.Template.Child()
	version_column = Gtk.Template.Child()
	repository_column = Gtk.Template.Child()
	status_column = Gtk.Template.Child()
	date_column = Gtk.Template.Child()
	size_column = Gtk.Template.Child()
	group_column = Gtk.Template.Child()

	#-----------------------------------
	# Properties
	#-----------------------------------
	__current_status = PkgStatus.ALL

	@GObject.Property(type=int, default=PkgStatus.ALL)
	def current_status(self):
		return(self.__current_status)

	@current_status.setter
	def current_status(self, value):
		self.__current_status = value

		self.status_filter.changed(Gtk.FilterChange.DIFFERENT)

	__current_search = ""

	@GObject.Property(type=str, default="")
	def current_search(self):
		return(self.__current_search)

	@current_search.setter
	def current_search(self, value):
		self.__current_search = value.lower()

		self.search_filter.changed(Gtk.FilterChange.DIFFERENT)

	__search_by_name = True

	@GObject.Property(type=bool, default=True)
	def search_by_name(self):
		return(self.__search_by_name)

	@search_by_name.setter
	def search_by_name(self, value):
		self.__search_by_name = value

		self.search_filter.changed(Gtk.FilterChange.DIFFERENT)

	__search_by_desc = False

	@GObject.Property(type=bool, default=False)
	def search_by_desc(self):
		return(self.__search_by_desc)

	@search_by_desc.setter
	def search_by_desc(self, value):
		self.__search_by_desc = value

		self.search_filter.changed(Gtk.FilterChange.DIFFERENT)

	__search_by_group = False

	@GObject.Property(type=bool, default=False)
	def search_by_group(self):
		return(self.__search_by_group)

	@search_by_group.setter
	def search_by_group(self, value):
		self.__search_by_group = value

		self.search_filter.changed(Gtk.FilterChange.DIFFERENT)

	__search_by_deps = False

	@GObject.Property(type=bool, default=False)
	def search_by_deps(self):
		return(self.__search_by_deps)

	@search_by_deps.setter
	def search_by_deps(self, value):
		self.__search_by_deps = value

		self.search_filter.changed(Gtk.FilterChange.DIFFERENT)

	__search_by_optdeps = False

	@GObject.Property(type=bool, default=False)
	def search_by_optdeps(self):
		return(self.__search_by_optdeps)

	@search_by_optdeps.setter
	def search_by_optdeps(self, value):
		self.__search_by_optdeps = value

		self.search_filter.changed(Gtk.FilterChange.DIFFERENT)

	__search_by_provides = False

	@GObject.Property(type=bool, default=False)
	def search_by_provides(self):
		return(self.__search_by_provides)

	@search_by_provides.setter
	def search_by_provides(self, value):
		self.__search_by_provides = value

		self.search_filter.changed(Gtk.FilterChange.DIFFERENT)

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

		# Bind item count to empty label visibility
		self.selection.bind_property(
			"n-items", self.empty_label, "visible",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.DEFAULT,
			lambda binding, value: value == 0
		)

		# Bind column sorters to sort functions
		self.version_sorter.set_sort_func(self.sort_by_ver, "version")

		# Bind filters to filter functions
		self.status_filter.set_filter_func(self.filter_by_status)
		self.search_filter.set_filter_func(self.filter_by_search)

		# Sort view by name (first) column
		self.view.sort_by_column(self.view.get_columns()[0], Gtk.SortType.ASCENDING)

	#-----------------------------------
	# Sorter functions
	#-----------------------------------
	def sort_by_ver(self, item_a, item_b, prop):
		return(pyalpm.vercmp(item_a.get_property(prop), item_b.get_property(prop)))

	#-----------------------------------
	# Filter functions
	#-----------------------------------
	def filter_by_status(self, item):
		return(item.status_flags & self.current_status)

	def filter_by_search(self, item):
		if self.current_search == "":
			return(True)
		else:
			return(
				((self.current_search in item.name) if self.search_by_name else False)
				or
				((self.current_search in item.description.lower()) if self.search_by_desc else False)
				or
				((self.current_search in item.group.lower()) if self.search_by_group else False)
				or
				(([s for s in item.depends_list if self.current_search in s] != []) if self.search_by_deps else False)
				or
				(([s for s in item.optdepends_list if self.current_search in s] != []) if self.search_by_optdeps else False)
				or
				(([s for s in item.provides_list if self.current_search in s] != []) if self.search_by_provides else False)
			)

#------------------------------------------------------------------------------
#-- CLASS: SIDEBARLISTBOXROW
#------------------------------------------------------------------------------
@Gtk.Template(resource_path="/com/github/PacView/ui/sidebarlistboxrow.ui")
class SidebarListBoxRow(Gtk.ListBoxRow):
	__gtype_name__ = "SidebarListBoxRow"

	#-----------------------------------
	# Properties
	#-----------------------------------
	str_id = GObject.Property(type=str, default="")

	icon = GObject.Property(type=str, default="")
	text = GObject.Property(type=str, default="")

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

#------------------------------------------------------------------------------
#-- CLASS: SEARCHHEADER
#------------------------------------------------------------------------------
@Gtk.Template(resource_path="/com/github/PacView/ui/searchheader.ui")
class SearchHeader(Gtk.Stack):
	__gtype_name__ = "SearchHeader"

	#-----------------------------------
	# Class widget variables
	#-----------------------------------
	search_entry = Gtk.Template.Child()

	#-----------------------------------
	# Properties
	#-----------------------------------
	title = GObject.Property(type=str, default="")

	__key_capture_widget = None

	@GObject.Property(type=Gtk.Widget, default=None)
	def key_capture_widget(self):
		return(self.__key_capture_widget)

	@key_capture_widget.setter
	def key_capture_widget(self, value):
		self.__key_capture_widget = value

		self.search_entry.set_key_capture_widget(value)

	__search_active = False

	@GObject.Property(type=bool, default=False)
	def search_active(self):
		return(self.__search_active)

	@search_active.setter
	def search_active(self, value):
		self.__search_active = value

		if value == True:
			self.set_visible_child_name("search")

			self.search_entry.grab_focus()
		else:
			self.search_entry.set_text("")

			self.set_visible_child_name("title")

	search_term = GObject.Property(type=str, default="")

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

		# Bind entry text to search_term property
		self.search_entry.bind_property(
			"text", self, "search_term",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.DEFAULT
		)

	#-----------------------------------
	# Signal handlers
	#-----------------------------------
	@Gtk.Template.Callback()
	def on_search_started(self, entry):
		self.search_active = True

	@Gtk.Template.Callback()
	def on_search_stopped(self, entry):
		self.search_active = False

#------------------------------------------------------------------------------
#-- CLASS: MAINWINDOW
#------------------------------------------------------------------------------
@Gtk.Template(resource_path="/com/github/PacView/ui/mainwindow.ui")
class MainWindow(Adw.ApplicationWindow):
	__gtype_name__ = "MainWindow"

	#-----------------------------------
	# Class widget variables
	#-----------------------------------
	header_search = Gtk.Template.Child()

	header_sidebar_btn = Gtk.Template.Child()
	header_infopane_btn = Gtk.Template.Child()
	header_search_btn = Gtk.Template.Child()
	header_details_btn = Gtk.Template.Child()

	repo_listbox = Gtk.Template.Child()
	status_listbox = Gtk.Template.Child()

	pane = Gtk.Template.Child()

	column_view = Gtk.Template.Child()
	info_pane = Gtk.Template.Child()

	status_count_label = Gtk.Template.Child()

	update_image = Gtk.Template.Child()
	update_label = Gtk.Template.Child()

	status_search_box = Gtk.Template.Child()
	status_search_label_name = Gtk.Template.Child()
	status_search_label_desc = Gtk.Template.Child()
	status_search_label_group = Gtk.Template.Child()
	status_search_label_deps = Gtk.Template.Child()
	status_search_label_optdeps = Gtk.Template.Child()
	status_search_label_provides = Gtk.Template.Child()

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

		# Bind gsettings
		self.settings = Gio.Settings(schema_id="com.github.PacView")

		self.settings.bind("window-width", self, "default-width", Gio.SettingsBindFlags.DEFAULT)
		self.settings.bind("window-height", self, "default-height", Gio.SettingsBindFlags.DEFAULT)
		self.settings.bind("window-maximized", self, "maximized",Gio.SettingsBindFlags.DEFAULT)
		self.settings.bind("show-sidebar", self.header_sidebar_btn, "active",Gio.SettingsBindFlags.DEFAULT)
		self.settings.bind("show-infopane", self.header_infopane_btn, "active",Gio.SettingsBindFlags.DEFAULT)
		self.settings.bind("infopane-position", self.pane, "position",Gio.SettingsBindFlags.DEFAULT)
		self.settings.bind("show-column-version", self.column_view.version_column, "visible",Gio.SettingsBindFlags.DEFAULT)
		self.settings.bind("show-column-repository", self.column_view.repository_column, "visible",Gio.SettingsBindFlags.DEFAULT)
		self.settings.bind("show-column-status", self.column_view.status_column, "visible",Gio.SettingsBindFlags.DEFAULT)
		self.settings.bind("show-column-date", self.column_view.date_column, "visible",Gio.SettingsBindFlags.DEFAULT)
		self.settings.bind("show-column-size", self.column_view.size_column, "visible",Gio.SettingsBindFlags.DEFAULT)
		self.settings.bind("show-column-group", self.column_view.group_column, "visible",Gio.SettingsBindFlags.DEFAULT)

		# Gsettings actions
		self.add_action(self.settings.create_action("show-sidebar"))
		self.add_action(self.settings.create_action("show-infopane"))

		self.add_action(self.settings.create_action("show-column-version"))
		self.add_action(self.settings.create_action("show-column-repository"))
		self.add_action(self.settings.create_action("show-column-status"))
		self.add_action(self.settings.create_action("show-column-date"))
		self.add_action(self.settings.create_action("show-column-size"))
		self.add_action(self.settings.create_action("show-column-group"))

		# Bind package column view selected item to info pane
		self.column_view.selection.bind_property(
			"selected-item", self.info_pane, "pkg_object",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.DEFAULT
		)

		# Bind info pane package to details button enabled state
		self.info_pane.bind_property(
			"pkg_object", self.header_details_btn, "sensitive",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.DEFAULT,
			lambda binding, value: value is not None
		)

		# Bind package column view count to status label text
		self.column_view.filter_model.bind_property(
			"n-items", self.status_count_label, "label",
			GObject.BindingFlags.SYNC_CREATE | GObject.BindingFlags.DEFAULT,
			lambda binding, value: f'{value} matching package{"s" if value != 1 else ""}'
		)

		# Add actions
		action_list = [
			( "search-start", self.start_search_action ),
			( "search-stop", self.stop_search_action ),

			( "search-reset-params", self.reset_search_params_action ),

			( "view-prev-package", self.view_prev_package_action ),
			( "view-next-package", self.view_next_package_action ),
			( "show-details-window", self.show_details_window_action ),

			( "refresh-dbs", self.refresh_dbs_action ),
			( "show-stats-window", self.show_stats_window_action ),
			( "copy-package-list", self.copy_package_list_action ),

			( "show-about", self.show_about_action ),
			( "quit-app", self.quit_app_action )
		]

		self.add_action_entries(action_list)

		# Add property actions
		self.add_action(Gio.PropertyAction.new("search-by-name", self.column_view, "search_by_name"))
		self.add_action(Gio.PropertyAction.new("search-by-desc", self.column_view, "search_by_desc"))
		self.add_action(Gio.PropertyAction.new("search-by-group", self.column_view, "search_by_group"))
		self.add_action(Gio.PropertyAction.new("search-by-deps", self.column_view, "search_by_deps"))
		self.add_action(Gio.PropertyAction.new("search-by-optdeps", self.column_view, "search_by_optdeps"))
		self.add_action(Gio.PropertyAction.new("search-by-provides", self.column_view, "search_by_provides"))

		# Add action keyboard shortcuts
		app.set_accels_for_action("win.show-sidebar", ["<ctrl>b"])
		app.set_accels_for_action("win.show-infopane", ["<ctrl>i"])

		app.set_accels_for_action("win.search-start", ["<ctrl>f"])
		app.set_accels_for_action("win.search-stop", ["Escape"])

		app.set_accels_for_action("win.search-by-name", ["<ctrl>1"])
		app.set_accels_for_action("win.search-by-desc", ["<ctrl>2"])
		app.set_accels_for_action("win.search-by-group", ["<ctrl>3"])
		app.set_accels_for_action("win.search-by-deps", ["<ctrl>4"])
		app.set_accels_for_action("win.search-by-optdeps", ["<ctrl>5"])
		app.set_accels_for_action("win.search-by-provides", ["<ctrl>6"])

		app.set_accels_for_action("win.search-reset-params", ["<ctrl>R"])

		app.set_accels_for_action("win.view-prev-package", ["<alt>Left"])
		app.set_accels_for_action("win.view-next-package", ["<alt>Right"])
		app.set_accels_for_action("win.show-details-window", ["Return", "KP_Enter"])

		app.set_accels_for_action("win.refresh-dbs", ["F5"])
		app.set_accels_for_action("win.show-stats-window", ["<alt>S"])
		app.set_accels_for_action("win.copy-package-list", ["<alt>L"])
		
		app.set_accels_for_action("win.show-about", ["F1"])
		app.set_accels_for_action("win.quit-app", ["<ctrl>q"])

		# Set initial focus on package column view
		self.set_focus(self.column_view.view)

	#-----------------------------------
	# Show window signal handler
	#-----------------------------------
	@Gtk.Template.Callback()
	def on_show(self, window):
		self.init_databases()
		self.populate_sidebar()
		self.populate_column_view()

	#-----------------------------------
	# Init databases function
	#-----------------------------------
	def init_databases(self):
		# Define sync database names
		self.sync_db_names = ["core", "extra", "community", "multilib"]

		# Get list of configured database names
		dbs = subprocess.run(shlex.split(f'pacman-conf -l'), stdout=subprocess.PIPE, stderr=subprocess.PIPE)

		self.pacman_db_names = [n for n in dbs.stdout.decode().split('\n') if n != ""]

		# Add AUR to configured database names
		self.pacman_db_names.append("AUR")

	#-----------------------------------
	# Init sidebar function
	#-----------------------------------
	def populate_sidebar(self):
		# Remove rows from listboxes
		while(row := self.repo_listbox.get_row_at_index(0)):
			self.repo_listbox.remove(row)

		while(row := self.status_listbox.get_row_at_index(0)):
			self.status_listbox.remove(row)

		# Add rows to repository list box
		repo_row = SidebarListBoxRow(icon="repository-symbolic", text="All")
		self.repo_listbox.append(repo_row)

		for db in self.pacman_db_names:
			self.repo_listbox.append(SidebarListBoxRow(icon="repository-symbolic", text=db if db.isupper() else str.title(db), str_id=db))

		# Add rows to status list box
		status_row = None

		for st in [PkgStatus.ALL, PkgStatus.INSTALLED, PkgStatus.EXPLICIT, PkgStatus.DEPENDENCY, PkgStatus.OPTIONAL, PkgStatus.ORPHAN, PkgStatus.NONE, PkgStatus.UPDATES]:
			row = SidebarListBoxRow(icon="status-symbolic", text=st.name.title(), str_id=st.value)
			self.status_listbox.append(row)
			if st == PkgStatus.INSTALLED: status_row = row

		# Select initial repo/status
		self.repo_listbox.select_row(repo_row)
		self.status_listbox.select_row(status_row)

	#-----------------------------------
	# Populate column view functions
	#-----------------------------------
	def populate_column_view(self):
		# Get pyalpm handle
		alpm_handle = pyalpm.Handle("/", "/var/lib/pacman")

		# Package dict
		all_pkg_dict = {}

		# Add sync packages
		for db in self.pacman_db_names:
			sync_db = alpm_handle.register_syncdb(db, pyalpm.SIG_DATABASE_OPTIONAL)

			if sync_db is not None:
				all_pkg_dict.update(dict([(pkg.name, pkg) for pkg in sync_db.pkgcache]))

		# Add local packages
		local_db = alpm_handle.get_localdb()
		local_pkg_dict = dict([(pkg.name, pkg) for pkg in local_db.pkgcache])

		all_pkg_dict.update(dict([(pkg.name, pkg) for pkg in local_db.pkgcache if pkg.name not in all_pkg_dict.keys()]))

		# Create list of package objects
		def __get_local_data(name):
			if name in local_pkg_dict.keys():
				local_pkg = local_pkg_dict[name]

				status_flags = PkgStatus.NONE

				if local_pkg.reason == 0: status_flags = PkgStatus.EXPLICIT
				else:
					if local_pkg.compute_requiredby() != []:
						status_flags = PkgStatus.DEPENDENCY
					else:
						status_flags = PkgStatus.OPTIONAL if local_pkg.compute_optionalfor() != [] else PkgStatus.ORPHAN

				return(local_pkg, status_flags)

			return(None, PkgStatus.NONE)

		self.pkg_objects = [PkgObject(pkg, __get_local_data(pkg.name)) for pkg in all_pkg_dict.values()]

		self.column_view.model.splice(0, len(self.column_view.model), self.pkg_objects)

		# Add threaded function to get package updates
		thread = threading.Thread(target=self.checkupdates_async, daemon=True)
		thread.start()

		return(False)

	def checkupdates_async(self):
		# Get updates
		upd = subprocess.run(shlex.split(f'checkupdates'), stdout=subprocess.PIPE, stderr=subprocess.PIPE)

		expr = re.compile("(\S+)\s(\S+\s->\s\S+)")

		updates = {expr.sub(r"\1", u): expr.sub(r"\2", u) for u in upd.stdout.decode().split('\n') if u != ""}

		GLib.idle_add(self.show_pkg_updates, updates, upd.returncode)

	def show_pkg_updates(self, updates, returncode):
		if returncode == 0:
			# Modify package object properties if update available
			if len(updates) != 0:
				for obj in self.pkg_objects:
					if obj.name in updates.keys():
						obj.has_updates = True
						obj.status_flags |= PkgStatus.UPDATES
						obj.update_version = updates[obj.name]

			# Force update of info pane package object
			self.info_pane.pkg_object = self.column_view.selection.get_selected_item()

			# Update status
			self.update_image.set_from_icon_name("pkg-update")
			self.update_label.set_label(f'{len(updates)} update{"s" if len(updates) != 1 else ""} available')
		elif returncode == 1:
			self.update_image.set_from_icon_name("error-update")
			self.update_label.set_label("Error retrieving updates")
		else:
			self.update_image.set_from_icon_name("no-update")
			self.update_label.set_label("No updates")

		return(False)

	#-----------------------------------
	# Action handlers
	#-----------------------------------
	def start_search_action(self, action, value, user_data):
		self.header_search.search_active = True

	def stop_search_action(self, action, value, user_data):
		self.header_search.search_active = False

		self.column_view.view.grab_focus()

	def reset_search_params_action(self, action, value, user_data):
		for n in ["name", "desc", "group", "deps", "optdeps", "provides"]:
			self.column_view.set_property(f'search_by_{n}', (n == "name"))
			
	def view_prev_package_action(self, action, value, user_data):
		self.info_pane.display_prev_package()

	def view_next_package_action(self, action, value, user_data):
		self.info_pane.display_next_package()

	def show_details_window_action(self, action, value, user_data):
		if self.info_pane.pkg_object is not None:
			details_window = PkgDetailsWindow(self.info_pane.pkg_object, transient_for=self)
			details_window.show()

	def refresh_dbs_action(self, action, value, user_data):
		self.header_search.search_active = False

		self.init_databases()
		self.populate_sidebar()
		GLib.idle_add(self.populate_column_view)

	def show_stats_window_action(self, action, value, user_data):
		stats_window = StatsWindow(transient_for=self)
		stats_window.show()

	def copy_package_list_action(self, action, value, user_data):
		copy_text = '\n'.join([f'{obj.repository}/{obj.name}' for obj in self.column_view.selection])

		clipboard = self.get_clipboard()

		content = Gdk.ContentProvider.new_for_value(GObject.Value(str, copy_text))

		clipboard.set_content(content)

	def show_about_action(self, action, value, user_data):
		about_window = Adw.AboutWindow(
			application_name="PacView",
			application_icon="software-properties",
			developer_name="draKKar1969",
			version="1.0.rc4",
			comments="A Pacman database and AUR browser for Arch Linux, heavily inspired by <a href='https://osdn.net/projects/pkgbrowser/'>PkgBrowser</a>",
			website="https://github.com/drakkar1969/pacview",
			developers=["draKKar1969"],
			designers=["draKKar1969"],
			license_type=Gtk.License.GPL_3_0,
			transient_for=self)

		about_window.show()

	def quit_app_action(self, action, value, user_data):
		self.close()

	#-----------------------------------
	# Signal handlers
	#-----------------------------------
	@Gtk.Template.Callback()
	def on_repo_selected(self, listbox, row):
		if row is not None:
			self.column_view.repo_filter.set_search(row.str_id)

	@Gtk.Template.Callback()
	def on_status_selected(self, listbox, row):
		if row is not None:
			self.column_view.current_status = PkgStatus(int(row.str_id))

#------------------------------------------------------------------------------
#-- CLASS: LAUNCHERAPP
#------------------------------------------------------------------------------
class LauncherApp(Adw.Application):
	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, **kwargs):
		super().__init__(**kwargs)

		# Connect signal handlers
		self.connect("activate", self.on_activate)

	#-----------------------------------
	# Signal handlers
	#-----------------------------------
	def on_activate(self, app):
		self.main_window = MainWindow(application=app)
		self.main_window.present()

#------------------------------------------------------------------------------
#-- MAIN APP
#------------------------------------------------------------------------------
app = LauncherApp(application_id="com.github.PacView")
app.run(sys.argv)
