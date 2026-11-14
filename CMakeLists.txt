cmake_minimum_required(VERSION 3.24)
project(3dtiles_editor LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)

find_package(Qt6 6.6 REQUIRED COMPONENTS Core Gui Quick QuickControls2 Widgets)

include(FetchContent)
FetchContent_Declare(
    Corrosion
    GIT_REPOSITORY https://github.com/corrosion-rs/corrosion.git
    GIT_TAG v0.5.0
)
FetchContent_MakeAvailable(Corrosion)

corrosion_import_crate(
    MANIFEST_PATH "${CMAKE_CURRENT_SOURCE_DIR}/Cargo.toml"
    CRATES tiles_ffi
)

qt_add_executable(3dtiles-editor
    qt_ui/src/main.cpp
    qt_ui/src/EngineBridge.cpp
    qt_ui/src/ViewportItem.cpp
    qt_ui/src/TileTreeModel.cpp
    qt_ui/include/EngineBridge.hpp
    qt_ui/include/ViewportItem.hpp
    qt_ui/include/TileTreeModel.hpp
)

qt_add_qml_module(3dtiles-editor
    URI EditorUI
    VERSION 1.0
    QML_FILES
        qt_ui/qml/Main.qml
        qt_ui/qml/SceneTreePanel.qml
        qt_ui/qml/InspectorPanel.qml
        qt_ui/qml/LogBar.qml
)

target_include_directories(3dtiles-editor PRIVATE
    "${CMAKE_CURRENT_SOURCE_DIR}/qt_ui/include"
    "${CMAKE_CURRENT_SOURCE_DIR}/crates/tiles_ffi/include"
)

target_link_libraries(3dtiles-editor PRIVATE
    Qt6::Core
    Qt6::Gui
    Qt6::Quick
    Qt6::QuickControls2
    Qt6::Widgets
    tiles_ffi
)

if(WIN32)
    target_link_libraries(tiles_ffi INTERFACE
        propsys
        runtimeobject
        ole32
        oleaut32
        d3d12
        dxgi
        d3dcompiler
        opengl32
        ws2_32
        userenv
        bcrypt
        ntdll
    )
    set_target_properties(3dtiles-editor PROPERTIES
        WIN32_EXECUTABLE TRUE
    )
endif()
