install(PROGRAMS com.goshapps.AppImageManager.desktop DESTINATION ${KDE_INSTALL_APPDIR})
install(FILES com.goshapps.AppImageManager.metainfo.xml DESTINATION ${KDE_INSTALL_METAINFODIR})
install(FILES icons/hicolor/scalable/apps/com.goshapps.AppImageManager.svg
        DESTINATION ${KDE_INSTALL_ICONDIR}/hicolor/scalable/apps)
install(FILES icons/hicolor/symbolic/apps/com.goshapps.AppImageManager-symbolic.svg
        DESTINATION ${KDE_INSTALL_ICONDIR}/hicolor/symbolic/apps)
install(FILES screenshots/library.png DESTINATION ${KDE_INSTALL_DATADIR}/gosh-appimage-manager/screenshots)
install(DIRECTORY ${CMAKE_SOURCE_DIR}/third_party/licenses DESTINATION ${KDE_INSTALL_DATADIR}/gosh-appimage-manager)
install(FILES ${CMAKE_SOURCE_DIR}/third_party/NOTICE DESTINATION ${KDE_INSTALL_DATADIR}/gosh-appimage-manager)
install(FILES gosh-appimage-manager.notifyrc DESTINATION ${KDE_INSTALL_KNOTIFYRCDIR})
