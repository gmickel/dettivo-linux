#include "upload_source.h"

#include <QFile>
#include <QFileInfo>

#include <cerrno>
#include <cstring>
#include <fcntl.h>
#include <unistd.h>

namespace dettivo {
namespace {
bool same(const struct stat &a, const struct stat &b)
{
    return a.st_dev == b.st_dev && a.st_ino == b.st_ino && a.st_size == b.st_size
        && a.st_mtim.tv_sec == b.st_mtim.tv_sec && a.st_mtim.tv_nsec == b.st_mtim.tv_nsec
        && a.st_ctim.tv_sec == b.st_ctim.tv_sec && a.st_ctim.tv_nsec == b.st_ctim.tv_nsec;
}
}  // namespace

QString importContentType(const QString &path)
{
    const QString ext = QFileInfo(path).suffix().toLower();
    if (ext == QStringLiteral("wav"))
        return QStringLiteral("audio/wav");
    if (ext == QStringLiteral("mp3"))
        return QStringLiteral("audio/mpeg");
    if (ext == QStringLiteral("mp4"))
        return QStringLiteral("audio/mp4");
    if (ext == QStringLiteral("m4a"))
        return QStringLiteral("audio/m4a");
    if (ext == QStringLiteral("aac"))
        return QStringLiteral("audio/aac");
    if (ext == QStringLiteral("caf"))
        return QStringLiteral("audio/caf");
    if (ext == QStringLiteral("aif") || ext == QStringLiteral("aiff"))
        return QStringLiteral("audio/aiff");
    if (ext == QStringLiteral("flac"))
        return QStringLiteral("audio/flac");
    if (ext == QStringLiteral("ogg") || ext == QStringLiteral("oga") || ext == QStringLiteral("opus"))
        return QStringLiteral("audio/ogg");
    return QString();
}

UploadSource::UploadSource(QString path) : m_path(std::move(path)) {}

UploadSource::~UploadSource()
{
    if (m_fd >= 0)
        ::close(m_fd);
}

QString UploadSource::open()
{
    m_fd = ::open(QFile::encodeName(m_path).constData(), O_RDONLY | O_CLOEXEC | O_NONBLOCK);
    if (m_fd < 0)
        return QStringLiteral("cannot read the recording: %1").arg(QString::fromLocal8Bit(std::strerror(errno)));
    if (::fstat(m_fd, &m_identity) != 0 || !S_ISREG(m_identity.st_mode))
        return QStringLiteral("the recording is not a readable regular file");
    if (!unchanged())
        return QStringLiteral("the recording changed while opening it");
    return {};
}

bool UploadSource::unchanged() const
{
    struct stat descriptor {}, path {};
    return ::fstat(m_fd, &descriptor) == 0 && ::stat(QFile::encodeName(m_path).constData(), &path) == 0
        && same(m_identity, descriptor) && same(m_identity, path);
}

UploadSource::Chunk UploadSource::next(qint64 chunkBytes)
{
    Chunk chunk;
    if (!unchanged()) {
        chunk.error = QStringLiteral("the recording changed during upload");
        return chunk;
    }
    if (chunkBytes <= 0 || chunkBytes > kChunkBytes) {
        chunk.error = QStringLiteral("invalid upload chunk limit");
        return chunk;
    }
    chunk.bytes.resize(chunkBytes);
    ssize_t count;
    do {
        count = ::read(m_fd, chunk.bytes.data(), size_t(chunk.bytes.size()));
    } while (count < 0 && errno == EINTR);
    if (count < 0) {
        chunk.bytes.clear();
        chunk.error = QStringLiteral("cannot read the recording: %1").arg(QString::fromLocal8Bit(std::strerror(errno)));
        return chunk;
    }
    chunk.bytes.resize(count);
    m_read += count;
    if (!unchanged() || m_read > size() || (count == 0 && m_read != size())) {
        chunk.bytes.clear();
        chunk.error = QStringLiteral("the recording changed during upload");
        return chunk;
    }
    m_hash.addData(chunk.bytes);
    chunk.done = count == 0;
    if (chunk.done)
        chunk.digest = QString::fromLatin1(m_hash.result().toHex());
    return chunk;
}

}  // namespace dettivo
