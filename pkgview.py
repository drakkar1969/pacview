#!/usr/bin/env python

import gi, sys
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
		if self.pkg.reason == 0:
			return("installed")
		else:
			if self.pkg.compute_requiredby() != []: return("dependency")
			else:
				return("optional" if self.pkg.compute_optionalfor() != [] else "orphan")

	def get_date(self):
		return(str(datetime.datetime.fromtimestamp(self.pkg.installdate)))

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
		self.name_factory.connect("setup", self.on_item_setup)
		self.name_factory.connect("bind", self.on_item_bind_name)
		self.version_factory.connect("setup", self.on_item_setup)
		self.version_factory.connect("bind", self.on_item_bind_version)
		self.repository_factory.connect("setup", self.on_item_setup)
		self.repository_factory.connect("bind", self.on_item_bind_repository)
		self.status_factory.connect("setup", self.on_item_setup)
		self.status_factory.connect("bind", self.on_item_bind_status)
		self.date_factory.connect("setup", self.on_item_setup)
		self.date_factory.connect("bind", self.on_item_bind_date)
		self.size_factory.connect("setup", self.on_item_setup)
		self.size_factory.connect("bind", self.on_item_bind_size)

	#-----------------------------------
	# Factory signal handlers
	#-----------------------------------
	def on_item_setup(self, factory, item):
		item.set_child(Gtk.Label(halign=Gtk.Align.START))

	def on_item_bind_name(self, factory, item):
		item.get_child().set_label(item.get_item().pkg.name)

	def on_item_bind_version(self, factory, item):
		item.get_child().set_label(item.get_item().pkg.version)

	def on_item_bind_repository(self, factory, item):
		item.get_child().set_label(item.get_item().pkg.db.name)

	def on_item_bind_status(self, factory, item):
		item.get_child().set_label(item.get_item().get_status())

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
	pkg_columnview = Gtk.Template.Child()

	#-----------------------------------
	# Init function
	#-----------------------------------
	def __init__(self, *args, **kwargs):
		super().__init__(*args, **kwargs)

		handle = pyalpm.Handle("/", "/var/lib/pacman")
		# # coredb = handle.register_syncdb("core", pyalpm.SIG_DATABASE_OPTIONAL)
		localdb = handle.get_localdb()

		pkg_objects = [PkgObject(pkg) for pkg in localdb.pkgcache]

		self.pkg_columnview.store.splice(0, 0, pkg_objects)

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
app = LauncherApp(application_id="com.github.PkgView")
app.run(sys.argv)
