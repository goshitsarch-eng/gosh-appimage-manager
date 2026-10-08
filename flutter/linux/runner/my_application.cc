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

// The desktop frame's content box (SPEC section 1: 1280 x 800 inside the
// mockup's 1 px border). The window has no native frame, so the Flutter title
// bar is the only chrome and the content starts at the window's top-left.
static const gint kDefaultWidth = 1280;
static const gint kDefaultHeight = 800;

// The narrow layout is 360 px wide; the window may shrink to that.
static const gint kMinWidth = 360;
static const gint kMinHeight = 480;

// The window's root pointer position and the event time, for an interactive
// move or resize started from a press on the Flutter title bar.
static void pointer_origin(GtkWindow* window, gint* root_x, gint* root_y,
                           guint32* time) {
  GdkDisplay* display = gtk_widget_get_display(GTK_WIDGET(window));
  GdkSeat* seat = gdk_display_get_default_seat(display);
  GdkDevice* pointer = gdk_seat_get_pointer(seat);
  gdk_device_get_position(pointer, nullptr, root_x, root_y);
  *time = gtk_get_current_event_time();
}

// Maps the names Dart sends to the GDK window edges.
static gboolean edge_from_name(const gchar* name, GdkWindowEdge* edge) {
  static const struct {
    const char* name;
    GdkWindowEdge edge;
  } kEdges[] = {
      {"north", GDK_WINDOW_EDGE_NORTH},
      {"south", GDK_WINDOW_EDGE_SOUTH},
      {"east", GDK_WINDOW_EDGE_EAST},
      {"west", GDK_WINDOW_EDGE_WEST},
      {"north-east", GDK_WINDOW_EDGE_NORTH_EAST},
      {"north-west", GDK_WINDOW_EDGE_NORTH_WEST},
      {"south-east", GDK_WINDOW_EDGE_SOUTH_EAST},
      {"south-west", GDK_WINDOW_EDGE_SOUTH_WEST},
  };
  for (const auto& entry : kEdges) {
    if (strcmp(name, entry.name) == 0) {
      *edge = entry.edge;
      return TRUE;
    }
  }
  return FALSE;
}

static gboolean is_maximized(GtkWindow* window) {
  GdkWindow* gdk_window = gtk_widget_get_window(GTK_WIDGET(window));
  return gdk_window != nullptr &&
         (gdk_window_get_state(gdk_window) & GDK_WINDOW_STATE_MAXIMIZED) != 0;
}

// Answers the "gosh/window" channel. The Flutter title bar sets the window
// title, minimizes, maximizes, closes and drags the window through here.
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
  } else if (strcmp(method, "minimize") == 0) {
    gtk_window_iconify(window);
    response = FL_METHOD_RESPONSE(fl_method_success_response_new(nullptr));
  } else if (strcmp(method, "toggleMaximize") == 0) {
    if (is_maximized(window)) {
      gtk_window_unmaximize(window);
    } else {
      gtk_window_maximize(window);
    }
    response = FL_METHOD_RESPONSE(fl_method_success_response_new(nullptr));
  } else if (strcmp(method, "close") == 0) {
    gtk_window_close(window);
    response = FL_METHOD_RESPONSE(fl_method_success_response_new(nullptr));
  } else if (strcmp(method, "startDrag") == 0) {
    gint root_x = 0;
    gint root_y = 0;
    guint32 time = 0;
    pointer_origin(window, &root_x, &root_y, &time);
    gtk_window_begin_move_drag(window, 1, root_x, root_y, time);
    response = FL_METHOD_RESPONSE(fl_method_success_response_new(nullptr));
  } else if (strcmp(method, "startResize") == 0) {
    GdkWindowEdge edge = GDK_WINDOW_EDGE_SOUTH_EAST;
    if (fl_value_get_type(args) == FL_VALUE_TYPE_STRING &&
        edge_from_name(fl_value_get_string(args), &edge)) {
      gint root_x = 0;
      gint root_y = 0;
      guint32 time = 0;
      pointer_origin(window, &root_x, &root_y, &time);
      gtk_window_begin_resize_drag(window, edge, 1, root_x, root_y, time);
      response = FL_METHOD_RESPONSE(fl_method_success_response_new(nullptr));
    } else {
      response = FL_METHOD_RESPONSE(fl_method_error_response_new(
          "bad-args", "startResize expects an edge name", nullptr));
    }
  } else {
    response = FL_METHOD_RESPONSE(fl_method_not_implemented_response_new());
  }
  fl_method_call_respond(method_call, response, nullptr);
}

// Tells Dart when the window is maximized or restored, so the title bar's
// maximize control shows the right state.
static gboolean on_window_state_event(GtkWidget* widget,
                                      GdkEventWindowState* event,
                                      gpointer user_data) {
  FlMethodChannel* channel = FL_METHOD_CHANNEL(user_data);
  if (channel != nullptr &&
      (event->changed_mask & GDK_WINDOW_STATE_MAXIMIZED) != 0) {
    const gboolean maximized =
        (event->new_window_state & GDK_WINDOW_STATE_MAXIMIZED) != 0;
    fl_method_channel_invoke_method(channel, "maximizedChanged",
                                    fl_value_new_bool(maximized), nullptr,
                                    nullptr, nullptr);
  }
  return FALSE;
}

// Implements GApplication::activate.
static void my_application_activate(GApplication* application) {
  MyApplication* self = MY_APPLICATION(application);
  GtkWindow* window =
      GTK_WINDOW(gtk_application_window_new(GTK_APPLICATION(application)));

  // No native frame and no GTK header bar: the Flutter title bar draws the
  // logo, the title and the window controls, as the mockup does.
  gtk_window_set_title(window, "Gosh AppImage Manager");
  gtk_window_set_decorated(window, FALSE);
  gtk_window_set_default_size(window, kDefaultWidth, kDefaultHeight);
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
  g_signal_connect(window, "window-state-event",
                   G_CALLBACK(on_window_state_event), window_channel);
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
  G_APPLICATION_CLASS(my_application_parent_class)->startup(application);
}

// Implements GApplication::shutdown.
static void my_application_shutdown(GApplication* application) {
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
