#pragma once

#include <QByteArray>
#include <QCryptographicHash>
#include <QString>

#include <sys/stat.h>

namespace dettivo {

QString importContentType(const QString &path);

// One owner calls open/next serially on a worker thread. At most one
// transfer chunk is retained; the source identity is checked around reads.
class UploadSource {
public:
    struct Chunk {
        QByteArray bytes;
        QString error, digest;
        bool done = false;
    };
    explicit UploadSource(QString path);
    ~UploadSource();
    UploadSource(const UploadSource &) = delete;
    UploadSource &operator=(const UploadSource &) = delete;
    QString open();
    Chunk next(qint64 chunkBytes);
    qint64 size() const { return m_identity.st_size; }
    static constexpr qint64 kChunkBytes = 1024 * 1024;

private:
    bool unchanged() const;
    QString m_path;
    int m_fd = -1;
    struct stat m_identity {};
    qint64 m_read = 0;
    QCryptographicHash m_hash{QCryptographicHash::Sha256};
};

}  // namespace dettivo
