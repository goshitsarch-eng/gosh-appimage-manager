#include "my_application.h"

#include <string.h>

#include <flutter_linux/flutter_linux.h>
#ifdef GDK_WINDOWING_X11
#include <gdk/gdkx.h>
#endif

#include "flutter/generated_plugin_registrant.h"

struct _MyApplication {
  GtkApplication parent_instance;
  char** dart_entrypoint_arguments;
};

G_DEFINE_TYPE(MyApplication, my_application, GTK_TYPE_APPLICATION)

// Called when first Flutter frame received.
static void first_frame_cb(MyApplication* self, FlView* view) {
  gtk_widget_show(gtk_widget_get_toplevel(GTK_WIDGET(view)));
}

// Minimum window size, matching the original application.
static const gint kMinWidth = 420;
static const gint kMinHeight = 420;

// Colours the header bar. Dart sends the window's background and text colours
// so the bar matches the page under it, as the original's header does. The
// provider is replaced on each call, so a change of appearance takes effect.
static void apply_chrome(GtkWindow* window, const gchar* background,
                         const gchar* foreground) {
  g_autofree gchar* css = g_strdup_printf(
      "headerbar { background-color: %s; background-image: none; "
      "border: none; box-shadow: none; color: %s; min-height: 48px; }\n"
      "headerbar button, headerbar label { color: %s; }",
      background, foreground, foreground);
  GtkCssProvider* provider = gtk_css_provider_new();
  g_autoptr(GError) error = nullptr;
  if (!gtk_css_provider_load_from_data(provider, css, -1, &error)) {
    g_warning("Cannot colour the header bar: %s", error->message);
    g_object_unref(provider);
    return;
  }
  GdkScreen* screen = gdk_screen_get_default();
  GtkCssProvider* previous = GTK_CSS_PROVIDER(
      g_object_get_data(G_OBJECT(window), "gosh-chrome-css"));
  if (previous != nullptr) {
    gtk_style_context_remove_provider_for_screen(screen,
                                                 GTK_STYLE_PROVIDER(previous));
  }
  gtk_style_context_add_provider_for_screen(
      screen, GTK_STYLE_PROVIDER(provider),
      GTK_STYLE_PROVIDER_PRIORITY_APPLICATION);
  // Setting the data drops (and so un-refs) the provider it replaces.
  g_object_set_data_full(G_OBJECT(window), "gosh-chrome-css", provider,
                         g_object_unref);
}

// Answers the "gosh/window" channel. Dart sets the window title to the page it
// is showing, as the original does, and the header bar colours.
static void window_method_call_cb(FlMethodChannel* channel,
                                  FlMethodCall* method_call,
                                  gpointer user_data) {
  GtkWindow* window = GTK_WINDOW(user_data);
  const gchar* method = fl_method_call_get_name(method_call);
  FlValue* args = fl_method_call_get_args(method_call);
  g_autoptr(FlMethodResponse) response = nullptr;
  if (strcmp(method, "setTitle") == 0) {
    if (fl_value_get_type(args) == FL_VALUE_TYPE_STRING) {
      gtk_window_set_title(window, fl_value_get_string(args));
      response = FL_METHOD_RESPONSE(fl_method_success_response_new(nullptr));
    } else {
      response = FL_METHOD_RESPONSE(fl_method_error_response_new(
          "bad-args", "setTitle expects a string", nullptr));
    }
  } else if (strcmp(method, "setChrome") == 0) {
    FlValue* background = nullptr;
    FlValue* foreground = nullptr;
    if (fl_value_get_type(args) == FL_VALUE_TYPE_MAP) {
      background = fl_value_lookup_string(args, "background");
      foreground = fl_value_lookup_string(args, "foreground");
    }
    if (background != nullptr && foreground != nullptr &&
        fl_value_get_type(background) == FL_VALUE_TYPE_STRING &&
        fl_value_get_type(foreground) == FL_VALUE_TYPE_STRING) {
      apply_chrome(window, fl_value_get_string(background),
                   fl_value_get_string(foreground));
      response = FL_METHOD_RESPONSE(fl_method_success_response_new(nullptr));
    } else {
      response = FL_METHOD_RESPONSE(fl_method_error_response_new(
          "bad-args", "setChrome expects background and foreground", nullptr));
    }
  } else {
    response = FL_METHOD_RESPONSE(fl_method_not_implemented_response_new());
  }
  fl_method_call_respond(method_call, response, nullptr);
}

// The header's navigation button. Dart shows or hides the navigation rail.
static void on_nav_toggle_clicked(GtkButton* button, gpointer user_data) {
  GtkWindow* window = GTK_WINDOW(user_data);
  FlMethodChannel* channel = FL_METHOD_CHANNEL(
      g_object_get_data(G_OBJECT(window), "gosh-window-channel"));
  if (channel != nullptr) {
    fl_method_channel_invoke_method(channel, "toggleNav", nullptr, nullptr,
                                    nullptr, nullptr);
  }
}

// Implements GApplication::activate.
static void my_application_activate(GApplication* application) {
  MyApplication* self = MY_APPLICATION(application);
  GtkWindow* window =
      GTK_WINDOW(gtk_application_window_new(GTK_APPLICATION(application)));

  // The header bar is the window's own title bar, as the original's COSMIC
  // header is. It carries the navigation toggle at its start and stays empty,
  // like the original's; the page is named in the window title the channel
  // sets.
  gtk_window_set_title(window, "Gosh AppImage Manager");
  GtkHeaderBar* header_bar = GTK_HEADER_BAR(gtk_header_bar_new());
  gtk_widget_show(GTK_WIDGET(header_bar));
  // An empty custom title, so GTK does not draw the window title in the bar.
  gtk_header_bar_set_custom_title(header_bar, gtk_label_new(nullptr));
  gtk_header_bar_set_show_close_button(header_bar, TRUE);
  GtkWidget* toggle =
      gtk_button_new_from_icon_name("sidebar-show-symbolic", GTK_ICON_SIZE_BUTTON);
  gtk_widget_set_tooltip_text(toggle, "Show or hide the navigation");
  gtk_widget_show(toggle);
  g_signal_connect(toggle, "clicked", G_CALLBACK(on_nav_toggle_clicked),
                   window);
  gtk_header_bar_pack_start(header_bar, toggle);
  gtk_window_set_titlebar(window, GTK_WIDGET(header_bar));

  gtk_window_set_default_size(window, 1024, 768);
  GdkGeometry geometry;
  geometry.min_width = kMinWidth;
  geometry.min_height = kMinHeight;
  gtk_window_set_geometry_hints(window, nullptr, &geometry, GDK_HINT_MIN_SIZE);

  g_autoptr(FlDartProject) project = fl_dart_project_new();
  fl_dart_project_set_dart_entrypoint_arguments(
      project, self->dart_entrypoint_arguments);

  FlView* view = fl_view_new(project);
  GdkRGBA background_color;
  // Background defaults to black, override it here if necessary, e.g. #00000000
  // for transparent.
  gdk_rgba_parse(&background_color, "#000000");
  fl_view_set_background_color(view, &background_color);
  gtk_widget_show(GTK_WIDGET(view));
  gtk_container_add(GTK_CONTAINER(window), GTK_WIDGET(view));

  // Show the window when Flutter renders.
  // Requires the view to be realized so we can start rendering.
  g_signal_connect_swapped(view, "first-frame", G_CALLBACK(first_frame_cb),
                           self);
  gtk_widget_realize(GTK_WIDGET(view));

  // The channel must outlive this function; the window owns it.
  g_autoptr(FlStandardMethodCodec) codec = fl_standard_method_codec_new();
  FlMethodChannel* window_channel = fl_method_channel_new(
      fl_engine_get_binary_messenger(fl_view_get_engine(view)), "gosh/window",
      FL_METHOD_CODEC(codec));
  fl_method_channel_set_method_call_handler(window_channel,
                                            window_method_call_cb, window,
                                            nullptr);
  g_object_set_data_full(G_OBJECT(window), "gosh-window-channel",
                         window_channel, g_object_unref);

  fl_register_plugins(FL_PLUGIN_REGISTRY(view));

  gtk_widget_grab_focus(GTK_WIDGET(view));
}

// Implements GApplication::local_command_line.
static gboolean my_application_local_command_line(GApplication* application,
                                                  gchar*** arguments,
                                                  int* exit_status) {
  MyApplication* self = MY_APPLICATION(application);
  // Strip out the first argument as it is the binary name.
  self->dart_entrypoint_arguments = g_strdupv(*arguments + 1);

  g_autoptr(GError) error = nullptr;
  if (!g_application_register(application, nullptr, &error)) {
    g_warning("Failed to register: %s", error->message);
    *exit_status = 1;
    return TRUE;
  }

  g_application_activate(application);
  *exit_status = 0;

  return TRUE;
}

// Implements GApplication::startup.
static void my_application_startup(GApplication* application) {
  // MyApplication* self = MY_APPLICATION(object);

  // Perform any actions required at application startup.

  G_APPLICATION_CLASS(my_application_parent_class)->startup(application);
}

// Implements GApplication::shutdown.
static void my_application_shutdown(GApplication* application) {
  // MyApplication* self = MY_APPLICATION(object);

  // Perform any actions required at application shutdown.

  G_APPLICATION_CLASS(my_application_parent_class)->shutdown(application);
}

// Implements GObject::dispose.
static void my_application_dispose(GObject* object) {
  MyApplication* self = MY_APPLICATION(object);
  g_clear_pointer(&self->dart_entrypoint_arguments, g_strfreev);
  G_OBJECT_CLASS(my_application_parent_class)->dispose(object);
}

static void my_application_class_init(MyApplicationClass* klass) {
  G_APPLICATION_CLASS(klass)->activate = my_application_activate;
  G_APPLICATION_CLASS(klass)->local_command_line =
      my_application_local_command_line;
  G_APPLICATION_CLASS(klass)->startup = my_application_startup;
  G_APPLICATION_CLASS(klass)->shutdown = my_application_shutdown;
  G_OBJECT_CLASS(klass)->dispose = my_application_dispose;
}

static void my_application_init(MyApplication* self) {}

MyApplication* my_application_new() {
  // Set the program name to the application ID, which helps various systems
  // like GTK and desktop environments map this running application to its
  // corresponding .desktop file. This ensures better integration by allowing
  // the application to be recognized beyond its binary name.
  g_set_prgname(APPLICATION_ID);

  return MY_APPLICATION(g_object_new(my_application_get_type(),
                                     "application-id", APPLICATION_ID, "flags",
                                     G_APPLICATION_NON_UNIQUE, nullptr));
}
