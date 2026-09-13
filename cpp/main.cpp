#include <QGuiApplication>
#include <QIcon>
#include <QPixmap>
#include <QQmlApplicationEngine>
#include <QQmlError>
#include <QQuickImageProvider>
#include <QQuickStyle>
#include <QSize>
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

int main(int argc, char* argv[])
{
    QGuiApplication app(argc, argv);
    QCoreApplication::setOrganizationName(QStringLiteral("Application Priority Setter"));
    QCoreApplication::setApplicationName(QStringLiteral("Application Priority Setter"));
    QQuickStyle::setStyle(QStringLiteral("Fusion"));

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

    return app.exec();
}
