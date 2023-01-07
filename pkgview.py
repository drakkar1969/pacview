#!/usr/bin/env python

import gi, sys, os
gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Gtk, Adw, Gio, GObject
import pyalpm, datetime

#------------------------------------------------------------------------------
#-- CLASS: PKGOBJECT
#------------------------------------------------------------------------------
class PkgObject(GObject.Object):
	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, pkg, *args, **kwargs):
		super().__init__(*args, **kwargs)

		self.pkg = pkg

	#-----------------------------------
	# Value getter functions
	#-----------------------------------
	def get_status(self):
		local_pkg = pyalpm.find_satisfier(app.local_db.pkgcache, self.pkg.name)
		
		if local_pkg is not None:
			if local_pkg.reason == 0:
				return("installed", "object-select")
			else:
				if local_pkg.compute_requiredby() != []: return("dependency", "object-select")
				else:
					return(("optional", "object-select") if local_pkg.compute_optionalfor() != [] else ("orphan", "object-select"))
		else:
			return("", "")

	def get_date(self):
		local_pkg = pyalpm.find_satisfier(app.local_db.pkgcache, self.pkg.name)

		if local_pkg is not None:
			return(str(datetime.datetime.fromtimestamp(local_pkg.installdate)))
		else:
			return("")

	def get_size(self):
		pkg_size = self.pkg.isize

		for unit in ['B', 'KiB', 'MiB', 'GiB', 'TiB', 'PiB']:
			if pkg_size < 1024.0 or unit == 'PiB':
				break
			pkg_size /= 1024.0
		
		return(f"{pkg_size:.1f} {unit}")

#------------------------------------------------------------------------------
#-- CLASS: PKGCOLUMNVIEW
#------------------------------------------------------------------------------
@Gtk.Template(filename="/home/drakkar/Github/pkgview/pkgcolumnview.ui")
class PkgColumnView(Gtk.ScrolledWindow):
	__gtype_name__ = "PkgColumnView"

	#-----------------------------------
	# Class widget variables
	#-----------------------------------
	view = Gtk.Template.Child()
	store = Gtk.Template.Child()
	name_factory = Gtk.Template.Child()
	version_factory = Gtk.Template.Child()
	repository_factory = Gtk.Template.Child()
	status_factory = Gtk.Template.Child()
	date_factory = Gtk.Template.Child()
	size_factory = Gtk.Template.Child()

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

		# Bind column factories to signals
		self.name_factory.connect("setup", self.on_item_setup_iconlabel)
		self.name_factory.connect("bind", self.on_item_bind_name)
		self.version_factory.connect("setup", self.on_item_setup_label)
		self.version_factory.connect("bind", self.on_item_bind_version)
		self.repository_factory.connect("setup", self.on_item_setup_label)
		self.repository_factory.connect("bind", self.on_item_bind_repository)
		self.status_factory.connect("setup", self.on_item_setup_iconlabel)
		self.status_factory.connect("bind", self.on_item_bind_status)
		self.date_factory.connect("setup", self.on_item_setup_label)
		self.date_factory.connect("bind", self.on_item_bind_date)
		self.size_factory.connect("setup", self.on_item_setup_label)
		self.size_factory.connect("bind", self.on_item_bind_size)

	#-----------------------------------
	# Factory signal handlers
	#-----------------------------------
	def on_item_setup_label(self, factory, item):
		item.set_child(Gtk.Label(halign=Gtk.Align.START))

	def on_item_setup_iconlabel(self, factory, item):
		box = Gtk.Box()
		box.set_spacing(6)
		box.append(Gtk.Image())
		box.append(Gtk.Label(halign=Gtk.Align.START))
		item.set_child(box)

	def on_item_bind_name(self, factory, item):
		item.get_child().get_first_child().set_from_icon_name("package-x-generic-symbolic")
		item.get_child().get_last_child().set_label(item.get_item().pkg.name)

	def on_item_bind_version(self, factory, item):
		item.get_child().set_label(item.get_item().pkg.version)

	def on_item_bind_repository(self, factory, item):
		item.get_child().set_label(item.get_item().pkg.db.name)

	def on_item_bind_status(self, factory, item):
		label, icon = item.get_item().get_status()
		item.get_child().get_first_child().set_from_icon_name(icon)
		item.get_child().get_last_child().set_label(label)

	def on_item_bind_date(self, factory, item):
		item.get_child().set_label(item.get_item().get_date())

	def on_item_bind_size(self, factory, item):
		item.get_child().set_label(item.get_item().get_size())

#------------------------------------------------------------------------------
#-- CLASS: MAINWINDOW
#------------------------------------------------------------------------------
@Gtk.Template(filename="/home/drakkar/Github/pkgview/mainwindow.ui")
class MainWindow(Adw.ApplicationWindow):
	__gtype_name__ = "MainWindow"

	#-----------------------------------
	# Class widget variables
	#-----------------------------------
	repo_listbox = Gtk.Template.Child()
	repolist_allrow = Gtk.Template.Child()

	status_listbox = Gtk.Template.Child()
	statuslist_installedrow = Gtk.Template.Child()

	pkg_columnview = Gtk.Template.Child()

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

		self.repo_listbox.select_row(self.repolist_allrow)
		self.status_listbox.select_row(self.statuslist_installedrow)

		self.pkg_columnview.store.splice(0, 0, app.pkg_objects)

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

		alpm_folder = "/var/lib/pacman"

		db_path = os.path.join(alpm_folder, "sync")

		db_files = list(os.listdir(db_path)) if os.path.exists(db_path) else []
		db_files = [os.path.basename(db).split(".")[0] for db in db_files]

		alpm_handle = pyalpm.Handle("/", alpm_folder)
		self.local_db = alpm_handle.get_localdb()

		self.pkg_objects = []

		for db in db_files:
			sync_db = alpm_handle.register_syncdb(db, pyalpm.SIG_DATABASE_OPTIONAL)

			if sync_db is not None:
				self.pkg_objects.extend([PkgObject(pkg) for pkg in sync_db.pkgcache])

	#-----------------------------------
	# Signal handlers
	#-----------------------------------
	def on_activate(self, app):
		self.main_window = MainWindow(application=app)
		self.main_window.present()

#------------------------------------------------------------------------------
#-- MAIN APP
#------------------------------------------------------------------------------
app = LauncherApp(application_id="com.github.PkgView")
app.run(sys.argv)
