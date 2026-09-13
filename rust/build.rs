use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(
        QmlModule::new("io.github.applicationprioritysetter").qml_file("../qml/Main.qml"),
    )
    .qt_module("QuickControls2")
    .files(["src/controller.rs"])
    .build();
}
