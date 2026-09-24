// dettivo-visual-diff: the perceptual comparison the visual-regression
// checks use (ADR 0011). Two renders match when their structure
// correlates: each image is reduced to a small grid (one cell is four
// baseline pixels at 2x), every cell scored by its contrast against the
// image's own background and by its saturation, and the two grids are
// compared by correlation, which is blind to absolute colour and to the
// contrast range, so a light theme compares with a dark baseline and a
// render in another monospace font stays above the threshold while a
// missing element, a moved border or a wrong layout falls below it. The
// candidate is stretched to the baseline's width first, so a font that
// runs a few percent wider still lines up, and each grid may sit one cell
// to either side of the other, across or down: a font with a different
// line height moves a list of rows by a fraction of a cell, and without the
// vertical search two rows a cell apart read as opposites. `--strict` is
// for a baseline that is an approved render of the same theme and scale,
// where colour and geometry are the contract: the two images must have the
// same size, and the colour of every pixel at the original dimensions must stay
// within `--colour-tolerance` of the baseline's (a moved accent or a
// palette swap fails while small raster colour noise stays within tolerance). `crop`
// cuts approved baselines out of a larger sheet.
//
//   dettivo-visual-diff compare <baseline.png> <candidate.png>
//       [--threshold 0.55] [--diff <out.png>] [--json]
//       [--strict [--colour-tolerance 0.06]]
//   dettivo-visual-diff crop <in.png> <x> <y> <w> <h> <out.png>
//   dettivo-visual-diff tile <spec.json> <out.png>
//       spec: {"title": "...", "columns": ["black-gold", ...], "rows": [{"label": "listening",
//              "cells": [{"path": "...png", "caption": "0.61"} | null, ...]}, ...], "cell_width": 320}
#include <QColor>
#include <QFile>
#include <QFont>
#include <QGuiApplication>
#include <QHash>
#include <QImage>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QPainter>
#include <QString>
#include <QVector>

#include <algorithm>
#include <cmath>
#include <cstdio>
#include <cstring>

namespace {

constexpr int kRows = 18;
constexpr int kShift = 1;
constexpr double kDefaultThreshold = 0.55;
// The strict colour grid: finer than the structure grid so a swatch or a
// control's fill is a cell of its own, coarse enough that antialiasing
// along glyph edges averages away.
constexpr double kDefaultColourTolerance = 0.06;

struct Grid {
    int columns = 0;
    QVector<double> contrast;    // per cell, luminance distance from the background
    QVector<double> saturation;  // per cell, chroma
};

double luminance(const QColor &c)
{
    return 0.2126 * c.redF() + 0.7152 * c.greenF() + 0.0722 * c.blueF();
}

double saturationOf(const QColor &c)
{
    const double hi = std::max({c.redF(), c.greenF(), c.blueF()});
    const double lo = std::min({c.redF(), c.greenF(), c.blueF()});
    return hi - lo;
}

// The most frequent colour (quantised) is the background.
QColor background(const QImage &image)
{
    QHash<quint32, int> counts;
    for (int y = 0; y < image.height(); ++y) {
        const auto *row = reinterpret_cast<const QRgb *>(image.constScanLine(y));
        for (int x = 0; x < image.width(); ++x) {
            const QRgb p = row[x];
            const quint32 key = (quint32(qRed(p) >> 3) << 16) | (quint32(qGreen(p) >> 3) << 8) | quint32(qBlue(p) >> 3);
            counts[key] += 1;
        }
    }
    quint32 best = 0;
    int bestCount = -1;
    for (auto it = counts.constBegin(); it != counts.constEnd(); ++it) {
        if (it.value() > bestCount) {
            bestCount = it.value();
            best = it.key();
        }
    }
    return QColor(int((best >> 16) & 31) << 3, int((best >> 8) & 31) << 3, int(best & 31) << 3);
}

Grid gridOf(const QImage &source, int columns)
{
    const QImage argb = source.convertToFormat(QImage::Format_ARGB32);
    const QColor bg = background(argb);
    const double bgL = luminance(bg);
    const QImage image = argb.scaled(columns, kRows, Qt::IgnoreAspectRatio, Qt::SmoothTransformation);
    Grid grid;
    grid.columns = columns;
    grid.contrast.reserve(columns * kRows);
    grid.saturation.reserve(columns * kRows);
    for (int y = 0; y < kRows; ++y) {
        for (int x = 0; x < columns; ++x) {
            const QColor c = image.pixelColor(x, y);
            grid.contrast.append(std::abs(luminance(c) - bgL));
            grid.saturation.append(saturationOf(c));
        }
    }
    return grid;
}

/// The grid width: the baseline's aspect at kRows high.
int columnsFor(const QImage &baseline)
{
    if (baseline.height() <= 0)
        return 1;
    return std::max(1, int(std::lround(double(baseline.width()) * kRows / baseline.height())));
}

/// Pearson correlation of two cell maps with `b` shifted by `shift` cells
/// across and `shiftY` cells down.
double correlation(const QVector<double> &a, const QVector<double> &b, int columns, int shift, int shiftY)
{
    double sumA = 0, sumB = 0;
    int n = 0;
    for (int y = 0; y < kRows; ++y) {
        const int by = y + shiftY;
        if (by < 0 || by >= kRows)
            continue;
        for (int x = 0; x < columns; ++x) {
            const int bx = x + shift;
            if (bx < 0 || bx >= columns)
                continue;
            sumA += a.at(y * columns + x);
            sumB += b.at(by * columns + bx);
            ++n;
        }
    }
    if (n == 0)
        return 0.0;
    const double meanA = sumA / n;
    const double meanB = sumB / n;
    double sxy = 0, sxx = 0, syy = 0;
    for (int y = 0; y < kRows; ++y) {
        const int by = y + shiftY;
        if (by < 0 || by >= kRows)
            continue;
        for (int x = 0; x < columns; ++x) {
            const int bx = x + shift;
            if (bx < 0 || bx >= columns)
                continue;
            const double da = a.at(y * columns + x) - meanA;
            const double db = b.at(by * columns + bx) - meanB;
            sxy += da * db;
            sxx += da * da;
            syy += db * db;
        }
    }
    if (sxx <= 0.0 || syy <= 0.0)
        return 0.0;
    return sxy / std::sqrt(sxx * syy);
}

struct Score {
    double overall = 0.0;
    double contrast = 0.0;
    double saturation = 0.0;
    int shift = 0;
    int shiftY = 0;
    int columns = 0;
    QVector<double> cellDiff;
};

Score compare(const Grid &a, const Grid &b)
{
    Score best;
    best.overall = -1.0;
    for (int shiftY = -kShift; shiftY <= kShift; ++shiftY) {
        for (int shift = -kShift; shift <= kShift; ++shift) {
            const double c = std::max(0.0, correlation(a.contrast, b.contrast, a.columns, shift, shiftY));
            const double s = std::max(0.0, correlation(a.saturation, b.saturation, a.columns, shift, shiftY));
            const double overall = 0.7 * c + 0.3 * s;
            if (overall > best.overall) {
                best.overall = overall;
                best.contrast = c;
                best.saturation = s;
                best.shift = shift;
                best.shiftY = shiftY;
            }
        }
    }
    best.columns = a.columns;
    // The heat map: contrast difference per cell at the chosen shift, each
    // map scaled to its own maximum so both themes read the same.
    const double maxA = *std::max_element(a.contrast.begin(), a.contrast.end());
    const double maxB = *std::max_element(b.contrast.begin(), b.contrast.end());
    best.cellDiff.resize(a.columns * kRows);
    for (int y = 0; y < kRows; ++y) {
        const int by = std::clamp(y + best.shiftY, 0, kRows - 1);
        for (int x = 0; x < a.columns; ++x) {
            const int bx = std::clamp(x + best.shift, 0, a.columns - 1);
            const double va = maxA > 0 ? a.contrast.at(y * a.columns + x) / maxA : 0.0;
            const double vb = maxB > 0 ? b.contrast.at(by * a.columns + bx) / maxB : 0.0;
            best.cellDiff[y * a.columns + x] = std::abs(va - vb);
        }
    }
    return best;
}

/// The largest per-pixel RGB distance at the original dimensions.
/// The existing tolerance permits small raster colour noise, never shifts or rescaling.
double colourDistance(const QImage &baseline, const QImage &candidate)
{
    const QImage a = baseline.convertToFormat(QImage::Format_ARGB32);
    const QImage b = candidate.convertToFormat(QImage::Format_ARGB32);
    double worst = 0.0;
    for (int y = 0; y < a.height(); ++y) {
        for (int x = 0; x < a.width(); ++x) {
            const QColor ca = a.pixelColor(x, y);
            const QColor cb = b.pixelColor(x, y);
            const double dr = ca.redF() - cb.redF();
            const double dg = ca.greenF() - cb.greenF();
            const double db = ca.blueF() - cb.blueF();
            worst = std::max(worst, std::sqrt((dr * dr + dg * dg + db * db) / 3.0));
        }
    }
    return worst;
}

// Baseline, candidate and the cell differences stacked for a person.
void writeDiff(const QString &path, const QImage &baseline, const QImage &candidate, const Score &score)
{
    const int width = std::max(baseline.width(), candidate.width());
    const int heat = 54;
    QImage out(width, baseline.height() + candidate.height() + heat + 8, QImage::Format_ARGB32);
    out.fill(Qt::black);
    QPainter p(&out);
    p.drawImage(0, 0, baseline);
    p.drawImage(0, baseline.height() + 4, candidate);
    const double cellW = double(baseline.width()) / score.columns;
    const double cellH = double(heat) / kRows;
    const int top = baseline.height() + candidate.height() + 8;
    for (int y = 0; y < kRows; ++y) {
        for (int x = 0; x < score.columns; ++x) {
            const double d = std::min(1.0, score.cellDiff.at(y * score.columns + x) * 2.0);
            p.fillRect(QRectF(x * cellW, top + y * cellH, cellW + 0.5, cellH + 0.5),
                       QColor::fromRgbF(float(d), float(1.0 - d), 0.0f));
        }
    }
    p.end();
    out.save(path);
}

int runCompare(int argc, char **argv)
{
    if (argc < 4) {
        std::fprintf(stderr, "usage: dettivo-visual-diff compare <baseline.png> <candidate.png> [--threshold t] [--diff out.png] [--json] [--strict [--colour-tolerance t]]\n");
        return 2;
    }
    double threshold = kDefaultThreshold;
    double colourTolerance = kDefaultColourTolerance;
    QString diffPath;
    bool json = false;
    bool strict = false;
    for (int i = 4; i < argc; ++i) {
        if (std::strcmp(argv[i], "--threshold") == 0 && i + 1 < argc)
            threshold = std::atof(argv[++i]);
        else if (std::strcmp(argv[i], "--colour-tolerance") == 0 && i + 1 < argc)
            colourTolerance = std::atof(argv[++i]);
        else if (std::strcmp(argv[i], "--diff") == 0 && i + 1 < argc)
            diffPath = QString::fromLocal8Bit(argv[++i]);
        else if (std::strcmp(argv[i], "--json") == 0)
            json = true;
        else if (std::strcmp(argv[i], "--strict") == 0)
            strict = true;
    }
    const QImage baseline(QString::fromLocal8Bit(argv[2]));
    const QImage candidate(QString::fromLocal8Bit(argv[3]));
    if (baseline.isNull()) {
        std::fprintf(stderr, "dettivo-visual-diff: cannot read baseline %s\n", argv[2]);
        return 2;
    }
    if (candidate.isNull()) {
        std::fprintf(stderr, "dettivo-visual-diff: cannot read candidate %s\n", argv[3]);
        return 2;
    }
    const int columns = columnsFor(baseline);
    const Score score = compare(gridOf(baseline, columns), gridOf(candidate, columns));
    // The strict facts are measured whenever the sizes allow, so a report
    // can show them; they decide the verdict only under --strict.
    const bool sizeMatch = baseline.size() == candidate.size();
    const double colour = sizeMatch ? colourDistance(baseline, candidate) : 1.0;
    const bool strictPass = sizeMatch && colour <= colourTolerance;
    const bool pass = strict ? strictPass : score.overall >= threshold;
    if (!diffPath.isEmpty())
        writeDiff(diffPath, baseline, candidate, score);
    if (json) {
        const QJsonObject o{{QStringLiteral("baseline"), QString::fromLocal8Bit(argv[2])},
                            {QStringLiteral("candidate"), QString::fromLocal8Bit(argv[3])},
                            {QStringLiteral("score"), score.overall},
                            {QStringLiteral("contrast"), score.contrast},
                            {QStringLiteral("saturation"), score.saturation},
                            {QStringLiteral("shift"), score.shift},
                            {QStringLiteral("shift_y"), score.shiftY},
                            {QStringLiteral("threshold"), threshold},
                            {QStringLiteral("size_match"), sizeMatch},
                            {QStringLiteral("colour_distance"), colour},
                            {QStringLiteral("colour_tolerance"), colourTolerance},
                            {QStringLiteral("strict"), strict},
                            {QStringLiteral("pass"), pass}};
        std::printf("%s\n", QJsonDocument(o).toJson(QJsonDocument::Compact).constData());
    } else {
        std::printf("%s score %.3f (threshold %.2f; structure %.3f, colour %.3f, shift %d,%d; size %s, colour distance %.3f%s)\n",
                    pass ? "match" : "DIFFERENT", score.overall, threshold, score.contrast, score.saturation,
                    score.shift, score.shiftY, sizeMatch ? "same" : "DIFFERENT", colour,
                    strict ? (strictPass ? ", within tolerance" : ", over tolerance") : "");
    }
    return pass ? 0 : 1;
}

int runCrop(int argc, char **argv)
{
    if (argc != 8) {
        std::fprintf(stderr, "usage: dettivo-visual-diff crop <in.png> <x> <y> <w> <h> <out.png>\n");
        return 2;
    }
    const QImage in(QString::fromLocal8Bit(argv[2]));
    if (in.isNull()) {
        std::fprintf(stderr, "dettivo-visual-diff: cannot read %s\n", argv[2]);
        return 2;
    }
    const QRect rect(std::atoi(argv[3]), std::atoi(argv[4]), std::atoi(argv[5]), std::atoi(argv[6]));
    if (!in.rect().contains(rect)) {
        std::fprintf(stderr, "dettivo-visual-diff: crop %dx%d+%d+%d leaves the %dx%d image\n", rect.width(),
                     rect.height(), rect.x(), rect.y(), in.width(), in.height());
        return 2;
    }
    return in.copy(rect).save(QString::fromLocal8Bit(argv[7])) ? 0 : 1;
}

// One contact sheet: the title on top, a column head per theme, a row per
// state with its label on the left, every cell the render scaled to the
// cell width with its caption under it. A cell whose image is missing is
// drawn as an outlined box that says so.
int runTile(int argc, char **argv)
{
    if (argc != 4) {
        std::fprintf(stderr, "usage: dettivo-visual-diff tile <spec.json> <out.png>\n");
        return 2;
    }
    QFile specFile(QString::fromLocal8Bit(argv[2]));
    if (!specFile.open(QIODevice::ReadOnly)) {
        std::fprintf(stderr, "dettivo-visual-diff: cannot read %s\n", argv[2]);
        return 2;
    }
    QJsonParseError parseError{};
    const QJsonObject spec = QJsonDocument::fromJson(specFile.readAll(), &parseError).object();
    if (parseError.error != QJsonParseError::NoError || spec.isEmpty()) {
        std::fprintf(stderr, "dettivo-visual-diff: %s is not a tile spec (%s)\n", argv[2],
                     qUtf8Printable(parseError.errorString()));
        return 2;
    }
    const QJsonArray columns = spec.value(QStringLiteral("columns")).toArray();
    const QJsonArray rows = spec.value(QStringLiteral("rows")).toArray();
    if (columns.isEmpty() || rows.isEmpty()) {
        std::fprintf(stderr, "dettivo-visual-diff: the tile spec has no columns or no rows\n");
        return 2;
    }
    const int cellWidth = std::max(80, spec.value(QStringLiteral("cell_width")).toInt(320));
    const int label = 150;
    const int gap = 12;
    const int caption = 18;
    const int head = 56;
    // Every row is as tall as its tallest scaled image.
    QVector<QVector<QImage>> images(rows.size());
    QVector<int> rowHeights(rows.size(), 0);
    for (int r = 0; r < rows.size(); ++r) {
        const QJsonArray cells = rows.at(r).toObject().value(QStringLiteral("cells")).toArray();
        images[r].resize(columns.size());
        for (int c = 0; c < columns.size() && c < cells.size(); ++c) {
            const QString path = cells.at(c).toObject().value(QStringLiteral("path")).toString();
            if (path.isEmpty())
                continue;
            QImage image(path);
            if (image.isNull())
                continue;
            images[r][c] = image.scaledToWidth(cellWidth, Qt::SmoothTransformation);
            rowHeights[r] = std::max(rowHeights[r], images[r][c].height());
        }
        rowHeights[r] = std::max(rowHeights[r], 40);
    }
    int height = head + gap;
    for (int h : rowHeights)
        height += h + caption + gap;
    const int width = label + columns.size() * (cellWidth + gap) + gap;
    QImage out(width, height, QImage::Format_ARGB32);
    out.fill(QColor(24, 24, 24));
    QPainter p(&out);
    p.setRenderHint(QPainter::Antialiasing);
    QFont font = p.font();
    font.setPixelSize(12);
    p.setFont(font);
    p.setPen(QColor(230, 230, 230));
    p.drawText(QRect(gap, gap, width - 2 * gap, 20), Qt::AlignLeft | Qt::AlignVCenter,
               spec.value(QStringLiteral("title")).toString());
    for (int c = 0; c < columns.size(); ++c) {
        p.drawText(QRect(label + c * (cellWidth + gap), gap + 24, cellWidth, 20), Qt::AlignLeft | Qt::AlignVCenter,
                   columns.at(c).toString());
    }
    int y = head + gap;
    for (int r = 0; r < rows.size(); ++r) {
        const QJsonObject row = rows.at(r).toObject();
        const QJsonArray cells = row.value(QStringLiteral("cells")).toArray();
        p.setPen(QColor(230, 230, 230));
        p.drawText(QRect(gap, y, label - 2 * gap, rowHeights[r]), Qt::AlignLeft | Qt::AlignTop | Qt::TextWordWrap,
                   row.value(QStringLiteral("label")).toString());
        for (int c = 0; c < columns.size(); ++c) {
            const int x = label + c * (cellWidth + gap);
            const QRect cell(x, y, cellWidth, rowHeights[r]);
            if (images[r][c].isNull()) {
                p.setPen(QColor(120, 120, 120));
                p.drawRect(cell.adjusted(0, 0, -1, -1));
                p.drawText(cell, Qt::AlignCenter, QStringLiteral("no render"));
            } else {
                p.drawImage(x, y, images[r][c]);
                p.setPen(QColor(90, 90, 90));
                p.drawRect(QRect(x, y, images[r][c].width(), images[r][c].height()).adjusted(0, 0, -1, -1));
            }
            const QString text = c < cells.size() ? cells.at(c).toObject().value(QStringLiteral("caption")).toString() : QString();
            p.setPen(QColor(170, 170, 170));
            p.drawText(QRect(x, y + rowHeights[r], cellWidth, caption), Qt::AlignLeft | Qt::AlignVCenter, text);
        }
        y += rowHeights[r] + caption + gap;
    }
    p.end();
    if (!out.save(QString::fromLocal8Bit(argv[3]))) {
        std::fprintf(stderr, "dettivo-visual-diff: cannot write %s\n", argv[3]);
        return 1;
    }
    return 0;
}

}  // namespace

int main(int argc, char **argv)
{
    // The version answers before any GUI platform or accessibility bridge
    // comes up, so the line is the whole output.
    if (argc >= 2 && std::strcmp(argv[1], "--version") == 0) {
        std::printf("dettivo-visual-diff %s\n", DETTIVO_VERSION);
        return 0;
    }
    QGuiApplication app(argc, argv);
    if (argc >= 2 && std::strcmp(argv[1], "compare") == 0)
        return runCompare(argc, argv);
    if (argc >= 2 && std::strcmp(argv[1], "crop") == 0)
        return runCrop(argc, argv);
    if (argc >= 2 && std::strcmp(argv[1], "tile") == 0)
        return runTile(argc, argv);
    std::fprintf(stderr, "usage: dettivo-visual-diff compare|crop|tile ... (see the header of qt/tools/visual-diff/main.cpp)\n");
    return 2;
}
