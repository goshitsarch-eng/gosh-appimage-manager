add_library(goshaimcore STATIC
    core/ProcessRunner.cpp
    core/SafeFs.cpp
    core/ElfParser.cpp
    core/DesktopParser.cpp
    core/UrlGuard.cpp
    core/NetworkClient.cpp
    core/SettingsStore.cpp
    core/ManagedRegistry.cpp
    core/AppImageInspector.cpp
    core/DesktopIntegration.cpp
    core/IntegrationService.cpp
    core/ProcessTable.cpp
    core/RemovalLaunch.cpp
    core/UpdateSources.cpp
    core/UpdateService.cpp
    core/CheckStateStore.cpp
    core/UpdateNotifier.cpp
    core/AppImageLibrary.cpp
    core/TaskQueue.cpp
    models/Models.cpp
    AppController.cpp
    ThemeController.cpp
    Cli.cpp
)

target_include_directories(goshaimcore PUBLIC ${CMAKE_CURRENT_SOURCE_DIR} ${CMAKE_CURRENT_SOURCE_DIR}/..)
target_link_libraries(goshaimcore PUBLIC
    Qt6::Core
    Qt6::Gui
    Qt6::Network
    Qt6::Widgets
    Qt6::Qml
    Qt6::Quick
    KF6::I18n
    KF6::ConfigCore
    KF6::CoreAddons
    KF6::Notifications
)

qt_add_resources(goshaimcore "core_license"
    PREFIX "/gosh-appimage-manager"
    BASE "${CMAKE_SOURCE_DIR}"
    FILES
        ${CMAKE_SOURCE_DIR}/COPYING
)

add_executable(gosh-appimage-manager)
ecm_add_qml_module(gosh-appimage-manager URI com.goshapps.AppImageManager VERSION 1.0)

target_sources(gosh-appimage-manager PRIVATE
    main.cpp
)

ecm_target_qml_sources(gosh-appimage-manager SOURCES
    qml/Main.qml
    qml/LibraryPage.qml
    qml/DetailsPage.qml
    qml/InspectPage.qml
    qml/UpdatesPage.qml
    qml/TasksPage.qml
    qml/SettingsPage.qml
    qml/AboutPage.qml
    qml/StatusPage.qml
)

target_link_libraries(gosh-appimage-manager PRIVATE
    goshaimcore
    Qt6::Quick
    Qt6::Qml
    Qt6::Gui
    Qt6::QuickControls2
    Qt6::Widgets
    Qt6::Network
    Qt6::Svg
    KF6::I18n
    KF6::CoreAddons
    KF6::IconThemes
    KF6::ConfigCore
    KF6::DBusAddons
)

target_compile_definitions(gosh-appimage-manager PRIVATE HAVE_KDBUSADDONS)

install(TARGETS gosh-appimage-manager ${KDE_INSTALL_TARGETS_DEFAULT_ARGS})
