#include <cassert>
#include <numeric>
#include <vector>

#include "../calibrated_clustering.h"

int main() {
  assert(dettivo::LocalSpeakerCount(3.0f / 10, .4, 1.2) == 0);
  assert(dettivo::LocalSpeakerCount(4.0f / 10, .4, 1.2) == 1);
  assert(dettivo::LocalSpeakerCount(11.0f / 10, .4, 1.2) == 1);
  assert(dettivo::LocalSpeakerCount(12.0f / 10, .4, 1.2) == 2);
  assert(dettivo::LocalSpeakerCount(13.0f / 10, .5, 1.3) == 2);
  assert(dettivo::LocalSpeakerCount(14.0f / 10, .5, 1.4) == 2);
  assert(dettivo::LocalSpeakerCount(14.0f / 10, .5, 1.5) == 1);
  assert(dettivo::LocalSpeakerCount(15.0f / 10, .5, 1.5) == 2);
  std::vector<float> embeddings;
  std::vector<int> training(201);
  std::iota(training.begin(), training.end(), 0);
  for (int i = 0; i < 100; ++i) embeddings.insert(embeddings.end(), {1, 0});
  for (int i = 0; i < 100; ++i) embeddings.insert(embeddings.end(), {0, 1});
  embeddings.insert(embeddings.end(), {.9f, .1f});
  auto cluster = [](float *, int rows, int cols) {
    assert(rows == 201 && cols == 2);
    std::vector<int> labels(rows);
    for (int i = 0; i < rows; ++i) labels[i] = i < 100 ? 0 : (i < 200 ? 1 : 2);
    return labels;
  };
  auto labels = dettivo::ClusterAndAssign(embeddings.data(), 201, 2, training, cluster);
  for (int i = 0; i < 200; ++i) assert(labels[i] == (i < 100 ? 0 : 1));
  assert(labels[200] == 0);

  // Reliability filtering trains on the selected rows, but labels every row.
  training = {0, 100};
  assert(dettivo::MinimumPopulation(static_cast<int>(training.size())) == 1);
  labels = dettivo::ClusterAndAssign(embeddings.data(), 201, 2, training,
      [](float *features, int rows, int cols) {
        assert(rows == 2 && cols == 2);
        assert(features[0] == 1 && features[3] == 1);
        return std::vector<int>{0, 1};
      });
  assert(labels.size() == 201 && labels[0] == 0 && labels[100] == 1 && labels[200] == 0);

  // Short/all-overlap inputs with no reliable rows still use all embeddings.
  const float zero[] = {0, 0};
  labels = dettivo::ClusterAndAssign(zero, 1, 2, {},
      [](float *, int rows, int) { assert(rows == 1); return std::vector<int>{0}; });
  assert(labels == std::vector<int>{0});
  labels = dettivo::ClusterAndAssign(zero, 0, 2, {},
      [](float *, int, int) { assert(false); return std::vector<int>{}; });
  assert(labels.empty());

  assert(dettivo::MinimumPopulation(49) == 1);
  assert(dettivo::MinimumPopulation(150) == 2);
  assert(dettivo::MinimumPopulation(250) == 2);
  assert(dettivo::MinimumPopulation(350) == 4);
  assert(dettivo::MinimumPopulation(10000) == 15);

  embeddings.clear();
  training.resize(2016);
  assert(dettivo::MinimumPopulation(static_cast<int>(training.size())) == 15);
  std::iota(training.begin(), training.end(), 0);
  for (int i = 0; i < 1000; ++i) embeddings.insert(embeddings.end(), {1, 0});
  for (int i = 0; i < 1000; ++i) embeddings.insert(embeddings.end(), {std::sqrt(.75f), .5f});
  for (int i = 0; i < 16; ++i) embeddings.insert(embeddings.end(), {.5f, std::sqrt(.75f)});
  labels = dettivo::ClusterAndAssign(embeddings.data(), 2016, 2, training,
      [](float *, int rows, int cols) {
        assert(rows == 2016 && cols == 2);
        std::vector<int> groups(rows);
        for (int i = 0; i < rows; ++i) groups[i] = i < 1000 ? 0 : (i < 2000 ? 1 : 2);
        return groups;
      });
  for (int i = 0; i < 2016; ++i) assert(labels[i] == (i < 1000 ? 0 : 1));

  std::vector<float> centroids{0, 0, 1, 0};
  std::vector<int> population{16, 1000};
  dettivo::detail::ConsolidateCentroids(centroids, population, 2, 15);
  assert(centroids == std::vector<float>({0, 0, 1, 0}));
  assert(population == std::vector<int>({16, 1000}));
}
