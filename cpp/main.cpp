#include <QApplication>
#include <QIcon>
#include <QMenu>
#include <QAction>
#include <QPixmap>
#include <QQmlApplicationEngine>
#include <QQmlError>
#include <QQuickImageProvider>
#include <QQuickWindow>
#include <QSize>
#include <QSystemTrayIcon>
#include <QUrl>

#include <cstdio>

class ThemeIconProvider final : public QQuickImageProvider
{
public:
    ThemeIconProvider()
        : QQuickImageProvider(QQuickImageProvider::Pixmap)
    {
    }

    QPixmap requestPixmap(const QString& id, QSize* size, const QSize& requestedSize) override
    {
        const QSize targetSize = requestedSize.isValid() ? requestedSize : QSize(32, 32);
        const QPixmap pixmap = QIcon::fromTheme(id).pixmap(targetSize);
        if (size != nullptr)
            *size = pixmap.size();
        return pixmap;
    }
};

namespace
{

QIcon trayIcon()
{
    QIcon icon = QIcon::fromTheme(QStringLiteral("utilities-system-monitor"));
    if (icon.isNull())
        icon = QIcon::fromTheme(QStringLiteral("applications-system"));
    return icon;
}

void toggleWindow(QWindow* window)
{
    if (window == nullptr)
        return;
    if (window->isVisible()) {
        window->hide();
    } else {
        window->show();
        window->requestActivate();
        window->raise();
    }
}

} // namespace

int main(int argc, char* argv[])
{
    // QApplication (not QGuiApplication): required for QSystemTrayIcon.
    QApplication app(argc, argv);
    QCoreApplication::setOrganizationName(QStringLiteral("Application Priority Setter"));
    QCoreApplication::setApplicationName(QStringLiteral("Application Priority Setter"));
    app.setWindowIcon(trayIcon());

    // Default quitOnLastWindowClosed stays on: with close-to-tray enabled the
    // QML onClosing handler rejects the close (so the app keeps running),
    // otherwise closing the window quits as usual.

    // No forced QQuickStyle: Qt resolves the Controls style from
    // QT_QUICK_CONTROLS_STYLE / qtquickcontrols2.conf / the desktop session
    // (e.g. org.kde.desktop on Plasma), so the app follows system theming.

    QQmlApplicationEngine engine;
    engine.addImageProvider(QStringLiteral("theme"), new ThemeIconProvider);
    const QUrl url(QStringLiteral(
        "qrc:/qt/qml/io/github/applicationprioritysetter/qml/Main.qml"));
    QObject::connect(&engine, &QQmlEngine::warnings, &app, [](const QList<QQmlError>& warnings) {
        for (const QQmlError& warning : warnings)
            std::fprintf(stderr, "%s\n", qPrintable(warning.toString()));
    });
    QObject::connect(
        &engine,
        &QQmlApplicationEngine::objectCreationFailed,
        &app,
        [url] {
            std::fprintf(stderr, "Could not create the QML window from %s\n", qPrintable(url.toString()));
            QCoreApplication::exit(EXIT_FAILURE);
        },
        Qt::QueuedConnection);
    engine.load(url);
    if (engine.rootObjects().isEmpty())
        return EXIT_FAILURE;

    auto* window = qobject_cast<QQuickWindow*>(engine.rootObjects().constFirst());

    QSystemTrayIcon tray(trayIcon());
    tray.setToolTip(QStringLiteral("Application Priority Setter"));

    QMenu trayMenu;
    QAction* toggleAction = trayMenu.addAction(QStringLiteral("Hide"));
    QObject::connect(toggleAction, &QAction::triggered, &app, [window] {
        toggleWindow(window);
    });
    trayMenu.addSeparator();
    QAction* quitAction = trayMenu.addAction(QStringLiteral("Quit"));
    QObject::connect(quitAction, &QAction::triggered, &app, [window] {
        // Let the QML onClosing handler accept this one (close-to-tray
        // would otherwise swallow it and the app would never quit).
        if (window != nullptr)
            window->setProperty("allowQuit", true);
        QCoreApplication::quit();
    });
    tray.setContextMenu(&trayMenu);

    QObject::connect(
        &tray, &QSystemTrayIcon::activated, &app,
        [window](QSystemTrayIcon::ActivationReason reason) {
            if (reason == QSystemTrayIcon::Trigger)
                toggleWindow(window);
        });

    if (window != nullptr) {
        QObject::connect(window, &QWindow::visibleChanged, &app, [toggleAction](bool visible) {
            toggleAction->setText(visible ? QStringLiteral("Hide") : QStringLiteral("Show"));
        });
    }

    if (!QSystemTrayIcon::isSystemTrayAvailable())
        std::fprintf(stderr, "No system tray detected; running without a tray icon\n");
    tray.show();

    return app.exec();
}
