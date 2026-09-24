#include "unix_signals.h"

#include <QCoreApplication>
#include <QSocketNotifier>

#include <csignal>
#include <sys/socket.h>
#include <unistd.h>

namespace dettivo {

namespace {

int gSignalPipe[2] = {-1, -1};

void onSignal(int)
{
    const char byte = 1;
    // The write end is non-blocking; a full pipe means a quit is pending.
    [[maybe_unused]] const ssize_t n = ::write(gSignalPipe[1], &byte, 1);
}

}  // namespace

void quitOnTerminationSignals()
{
    if (gSignalPipe[0] != -1)
        return;
    if (::socketpair(AF_UNIX, SOCK_STREAM | SOCK_NONBLOCK | SOCK_CLOEXEC, 0, gSignalPipe) != 0)
        return;
    auto *notifier = new QSocketNotifier(gSignalPipe[0], QSocketNotifier::Read, QCoreApplication::instance());
    QObject::connect(notifier, &QSocketNotifier::activated, QCoreApplication::instance(), []() {
        char byte = 0;
        while (::read(gSignalPipe[0], &byte, 1) > 0) {
        }
        QCoreApplication::quit();
    });
    struct sigaction action {};
    action.sa_handler = onSignal;
    sigemptyset(&action.sa_mask);
    action.sa_flags = SA_RESTART;
    ::sigaction(SIGTERM, &action, nullptr);
    ::sigaction(SIGINT, &action, nullptr);
    ::sigaction(SIGHUP, &action, nullptr);
}

}  // namespace dettivo
