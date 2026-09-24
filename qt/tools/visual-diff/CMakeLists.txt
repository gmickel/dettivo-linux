# dettivo-visual-diff: the perceptual comparison behind the visual
# regression checks (ADR 0011), plus `crop` for cutting approved baselines
# out of a sheet. Built with the Qt tree; not installed.
qt_add_executable(dettivo-visual-diff main.cpp)
target_compile_definitions(dettivo-visual-diff PRIVATE DETTIVO_VERSION="${PROJECT_VERSION}")
target_link_libraries(dettivo-visual-diff PRIVATE Qt6::Core Qt6::Gui)

add_test(NAME dettivo-visual-diff-version COMMAND dettivo-visual-diff --version)
set_tests_properties(dettivo-visual-diff-version PROPERTIES
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen"
    PASS_REGULAR_EXPRESSION "^dettivo-visual-diff ${PROJECT_VERSION}")
# An image matches itself and differs from its inverse.
add_test(NAME dettivo-visual-diff-self
    COMMAND dettivo-visual-diff compare
        "${CMAKE_SOURCE_DIR}/../docs/design/studio/baselines/osd/listening.png"
        "${CMAKE_SOURCE_DIR}/../docs/design/studio/baselines/osd/listening.png"
        --threshold 0.999)
set_tests_properties(dettivo-visual-diff-self PROPERTIES ENVIRONMENT "QT_QPA_PLATFORM=offscreen")
# Under --strict an image matches itself (size and colour alike) and an
# approved 1x render fails against the 2x render of the same state: the
# size differs, however alike their structure is.
add_test(NAME dettivo-visual-diff-strict-self
    COMMAND dettivo-visual-diff compare
        "${CMAKE_SOURCE_DIR}/../docs/design/studio/baselines/osd/listening.builtin-dark.1x.png"
        "${CMAKE_SOURCE_DIR}/../docs/design/studio/baselines/osd/listening.builtin-dark.1x.png"
        --threshold 0.999 --strict --colour-tolerance 0)
set_tests_properties(dettivo-visual-diff-strict-self PROPERTIES ENVIRONMENT "QT_QPA_PLATFORM=offscreen")
add_test(NAME dettivo-visual-diff-strict-size
    COMMAND dettivo-visual-diff compare
        "${CMAKE_SOURCE_DIR}/../docs/design/studio/baselines/osd/listening.builtin-dark.1x.png"
        "${CMAKE_SOURCE_DIR}/../docs/design/studio/baselines/osd/listening.builtin-dark.2x.png"
        --threshold 0 --strict)
set_tests_properties(dettivo-visual-diff-strict-size PROPERTIES
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen"
    WILL_FAIL TRUE)
# The same state on the light palette against the dark one: the structure
# correlates (the pill is the pill) while the colour distance is large, so
# the plain comparison passes and the strict one fails.
add_test(NAME dettivo-visual-diff-strict-colour
    COMMAND dettivo-visual-diff compare
        "${CMAKE_SOURCE_DIR}/../docs/design/studio/baselines/osd/listening.builtin-dark.1x.png"
        "${CMAKE_SOURCE_DIR}/../docs/design/studio/baselines/osd/listening.builtin-light.1x.png"
        --threshold 0 --strict)
set_tests_properties(dettivo-visual-diff-strict-colour PROPERTIES
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen"
    WILL_FAIL TRUE)
add_test(NAME dettivo-visual-diff-other
    COMMAND dettivo-visual-diff compare
        "${CMAKE_SOURCE_DIR}/../docs/design/studio/baselines/osd/listening.png"
        "${CMAKE_SOURCE_DIR}/../docs/design/studio/baselines/osd/copied.png")
set_tests_properties(dettivo-visual-diff-other PROPERTIES
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen"
    WILL_FAIL TRUE)

add_test(NAME dettivo-visual-diff-strict-detail
    COMMAND python3 "${CMAKE_CURRENT_SOURCE_DIR}/test_strict.py" $<TARGET_FILE:dettivo-visual-diff>)
