// Observed platform, renderer and window metadata for the app's QA evidence.
#pragma once

#include <QGuiApplication>
#include <QJsonObject>
#include <QQuickWindow>
#include <QScreen>
#include <QSGRendererInterface>

namespace dettivo {

inline QJsonObject appEnvironment(const QQuickWindow *window)
{
    const auto api = window != nullptr ? window->rendererInterface()->graphicsApi() : QSGRendererInterface::Unknown;
    QString renderer;
    switch (api) {
    case QSGRendererInterface::OpenGL: renderer = QStringLiteral("OpenGL"); break;
    case QSGRendererInterface::Vulkan: renderer = QStringLiteral("Vulkan"); break;
    case QSGRendererInterface::Software: renderer = QStringLiteral("Software"); break;
    default: renderer = QStringLiteral("unknown"); break;
    }
#ifdef NDEBUG
    const QString build = QStringLiteral("Release");
#else
    const QString build = QStringLiteral("Debug");
#endif
    return {{QStringLiteral("qt_version"), QString::fromLatin1(qVersion())},
            {QStringLiteral("qt_build"), build},
            {QStringLiteral("platform"), QGuiApplication::platformName()},
            {QStringLiteral("renderer"), renderer},
            {QStringLiteral("graphics_api"), int(api)},
            {QStringLiteral("window_width"), window != nullptr ? window->width() : 0},
            {QStringLiteral("window_height"), window != nullptr ? window->height() : 0},
            {QStringLiteral("device_pixel_ratio"), window != nullptr ? window->devicePixelRatio() : 0.0},
            {QStringLiteral("screen"), window != nullptr && window->screen() != nullptr ? window->screen()->name() : QString()}};
}

}  // namespace dettivo
