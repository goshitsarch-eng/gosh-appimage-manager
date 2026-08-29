cmake_minimum_required(VERSION 3.20)

project(gosh-appimage-manager VERSION 0.1.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

find_package(ECM 6.10 REQUIRED NO_MODULE)
set(CMAKE_MODULE_PATH ${ECM_MODULE_PATH})

include(KDEInstallDirs)
include(KDECMakeSettings)
include(KDECompilerSettings NO_POLICY_SCOPE)
include(ECMFindQmlModule)
include(ECMQmlModule)
include(ECMInstallIcons)
include(CTest)
include(FeatureSummary)

find_package(Qt6 6.10 REQUIRED COMPONENTS Core Gui Qml Quick QuickControls2 Widgets Network Svg Test)
find_package(KF6 REQUIRED COMPONENTS Kirigami I18n CoreAddons Config IconThemes QQC2DesktopStyle DBusAddons)

ecm_find_qmlmodule(org.kde.kirigami REQUIRED)

add_subdirectory(src)
if(BUILD_TESTING)
    add_subdirectory(tests)
endif()
add_subdirectory(data)

ki18n_install(po)

feature_summary(WHAT ALL FATAL_ON_MISSING_REQUIRED_PACKAGES)
