include(ECMAddTests)

set(test_libs goshaimcore Qt6::Test Qt6::Core Qt6::Gui Qt6::Network Qt6::Widgets)

ecm_add_test(test_elf.cpp TEST_NAME test_elf LINK_LIBRARIES ${test_libs})
ecm_add_test(test_hash.cpp TEST_NAME test_hash LINK_LIBRARIES ${test_libs})
ecm_add_test(test_desktop.cpp TEST_NAME test_desktop LINK_LIBRARIES ${test_libs})
ecm_add_test(test_archive.cpp TEST_NAME test_archive LINK_LIBRARIES ${test_libs})
ecm_add_test(test_inspector.cpp TEST_NAME test_inspector LINK_LIBRARIES ${test_libs})
ecm_add_test(test_integration.cpp TEST_NAME test_integration LINK_LIBRARIES ${test_libs})
ecm_add_test(test_removal.cpp TEST_NAME test_removal LINK_LIBRARIES ${test_libs})
ecm_add_test(test_launch.cpp TEST_NAME test_launch LINK_LIBRARIES ${test_libs})
ecm_add_test(test_update.cpp TEST_NAME test_update LINK_LIBRARIES ${test_libs})
ecm_add_test(test_cli.cpp TEST_NAME test_cli LINK_LIBRARIES ${test_libs})
ecm_add_test(test_tasks.cpp TEST_NAME test_tasks LINK_LIBRARIES ${test_libs})
ecm_add_test(test_models.cpp TEST_NAME test_models LINK_LIBRARIES ${test_libs})
ecm_add_test(test_network.cpp TEST_NAME test_network LINK_LIBRARIES ${test_libs})
ecm_add_test(test_controller.cpp TEST_NAME test_controller LINK_LIBRARIES ${test_libs})

set_tests_properties(
    test_elf test_hash test_desktop test_archive test_inspector test_integration
    test_removal test_launch test_update test_cli test_tasks test_models test_network test_controller
    PROPERTIES ENVIRONMENT "QT_QPA_PLATFORM=offscreen")
