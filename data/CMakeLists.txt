install(PROGRAMS com.goshapps.AppImageManager.desktop DESTINATION ${KDE_INSTALL_APPDIR})
install(FILES com.goshapps.AppImageManager.metainfo.xml DESTINATION ${KDE_INSTALL_METAINFODIR})
install(FILES icons/hicolor/scalable/apps/com.goshapps.AppImageManager.svg
        DESTINATION ${KDE_INSTALL_ICONDIR}/hicolor/scalable/apps)
install(FILES icons/hicolor/symbolic/apps/com.goshapps.AppImageManager-symbolic.svg
        DESTINATION ${KDE_INSTALL_ICONDIR}/hicolor/symbolic/apps)
