#ifndef DETTIVO_CALIBRATED_CLUSTERING_H_
#define DETTIVO_CALIBRATED_CLUSTERING_H_

#include <algorithm>
#include <cmath>
#include <numeric>
#include <vector>

namespace dettivo {

inline int LocalSpeakerCount(float average, float voiced_threshold,
                             float overlap_threshold) {
  return average >= overlap_threshold ? 2 : (average >= voiced_threshold ? 1 : 0);
}

inline int MinimumPopulation(int training_rows) {
  int population = training_rows / 100;
  int remainder = training_rows % 100;
  if (remainder > 50 || (remainder == 50 && population % 2)) ++population;
  return std::clamp(population, 1, 15);
}

namespace detail {

inline void ConsolidateCentroids(std::vector<float> &centroids,
                                 std::vector<int> &population, int cols,
                                 int minimum) {
  while (population.size() > 1) {
    double best = INFINITY;
    size_t first = 0, second = 0;
    for (size_t a = 0; a < population.size(); ++a) {
      for (size_t b = a + 1; b < population.size(); ++b) {
        double dot = 0, norm_a = 0, norm_b = 0;
        for (int j = 0; j < cols; ++j) {
          const double x = centroids[a * cols + j], y = centroids[b * cols + j];
          dot += x * y;
          norm_a += x * x;
          norm_b += y * y;
        }
        if (!(norm_a > 0 && norm_b > 0)) continue;
        const double distance = std::clamp(1 - dot / std::sqrt(norm_a * norm_b), 0.0, 2.0);
        const double count_a = population[a], count_b = population[b];
        const double cost = 2 * distance * count_a * count_b / (count_a + count_b) / minimum;
        if (cost < best) {
          best = cost;
          first = a;
          second = b;
        }
      }
    }
    if (best > 1.5) break;
    const double count_a = population[first], count_b = population[second];
    for (int j = 0; j < cols; ++j) {
      centroids[first * cols + j] = static_cast<float>(
          (centroids[first * cols + j] * count_a + centroids[second * cols + j] * count_b) /
          (count_a + count_b));
    }
    population[first] += population[second];
    population.erase(population.begin() + second);
    centroids.erase(centroids.begin() + second * cols, centroids.begin() + (second + 1) * cols);
  }
}

}  // namespace detail

template <class Cluster>
std::vector<int> ClusterAndAssign(const float *features, int rows, int cols,
                                 std::vector<int> training, Cluster cluster) {
  if (rows <= 0 || cols <= 0) return {};
  if (training.size() < 2) {
    training.resize(rows);
    std::iota(training.begin(), training.end(), 0);
  }
  std::vector<float> selected(training.size() * cols);
  for (size_t i = 0; i < training.size(); ++i) {
    std::copy_n(features + training[i] * cols, cols, selected.data() + i * cols);
  }
  auto labels = cluster(selected.data(), static_cast<int>(training.size()), cols);
  if (labels.size() != training.size() ||
      std::any_of(labels.begin(), labels.end(), [&](int v) {
        return v < 0 || v >= static_cast<int>(training.size());
      })) return {};
  const int clusters = *std::max_element(labels.begin(), labels.end()) + 1;
  std::vector<int> population(clusters);
  for (int label : labels) ++population[label];
  const int minimum = MinimumPopulation(static_cast<int>(training.size()));
  std::vector<int> retained;
  for (int i = 0; i < clusters; ++i) {
    if (population[i] >= minimum) retained.push_back(i);
  }
  if (retained.empty()) {
    retained.push_back(std::max_element(population.begin(), population.end()) - population.begin());
  }
  std::vector<float> centroids(retained.size() * cols);
  std::vector<int> centroid_population;
  for (size_t c = 0; c < retained.size(); ++c) {
    centroid_population.push_back(population[retained[c]]);
    float *centroid = centroids.data() + c * cols;
    for (size_t i = 0; i < training.size(); ++i) {
      if (labels[i] != retained[c]) continue;
      for (int j = 0; j < cols; ++j) centroid[j] += features[training[i] * cols + j];
    }
    for (int j = 0; j < cols; ++j) {
      centroid[j] /= population[retained[c]];
    }
  }
  if (minimum > 1) detail::ConsolidateCentroids(centroids, centroid_population, cols, minimum);
  for (size_t c = 0; c < centroid_population.size(); ++c) {
    float *centroid = centroids.data() + c * cols;
    float norm = 0;
    for (int j = 0; j < cols; ++j) norm += centroid[j] * centroid[j];
    norm = std::sqrt(norm);
    if (norm > 0) for (int j = 0; j < cols; ++j) centroid[j] /= norm;
  }
  std::vector<int> assigned(rows);
  for (int i = 0; i < rows; ++i) {
    float best = -INFINITY;
    for (size_t c = 0; c < centroid_population.size(); ++c) {
      float similarity = 0;
      for (int j = 0; j < cols; ++j) similarity += features[i * cols + j] * centroids[c * cols + j];
      if (similarity > best) {
        best = similarity;
        assigned[i] = static_cast<int>(c);
      }
    }
  }
  return assigned;
}

}  // namespace dettivo

#endif
