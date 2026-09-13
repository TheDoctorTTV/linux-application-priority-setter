cmake_minimum_required(VERSION 3.24)

project(application-priority-setter VERSION 0.1.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)

find_package(Qt6 6.5 REQUIRED COMPONENTS Core Gui Qml Quick QuickControls2)

include(FetchContent)
find_package(CxxQt QUIET)
if(NOT CxxQt_FOUND)
    FetchContent_Declare(
        CxxQt
        GIT_REPOSITORY https://github.com/KDAB/cxx-qt-cmake.git
        GIT_TAG 0.10.0
    )
    FetchContent_MakeAvailable(CxxQt)
endif()

cxx_qt_import_crate(
    MANIFEST_PATH rust/Cargo.toml
    CRATES priority_setter_backend
    LOCKED
    QT_MODULES Qt::Core Qt::Gui Qt::Qml Qt::QuickControls2
)

cxx_qt_import_qml_module(priority_setter_qml
    URI io.github.applicationprioritysetter
    SOURCE_CRATE priority_setter_backend
)

qt_add_executable(application-priority-setter cpp/main.cpp)
target_link_libraries(application-priority-setter PRIVATE priority_setter_qml)
qt_import_qml_plugins(application-priority-setter)

install(TARGETS application-priority-setter RUNTIME DESTINATION bin)

