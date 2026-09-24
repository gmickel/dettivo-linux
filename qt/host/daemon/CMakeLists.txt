# What every Qt client of the daemon shares (ADR 0020): the JSON-RPC
# connection over the Unix socket with its event subscription and
# reconnect, the link interface the tests fake, and the path resolver that
# agrees with dettivod and the `dettivo` command on where the socket, the
# configuration, the state and the token live.
add_library(dettivo-daemon-client STATIC
    daemon_client.cpp
    daemon_client.h
    daemon_link.h
    daemon_paths.cpp
    daemon_paths.h
)
target_include_directories(dettivo-daemon-client PUBLIC "${CMAKE_CURRENT_SOURCE_DIR}")
target_link_libraries(dettivo-daemon-client PUBLIC Qt6::Core Qt6::Network)

# The client against a socket that accepts and never answers: every
# pending request fails once at its deadline and the connection stays up.
qt_add_executable(dettivo-daemon-client-test daemon_client_test.cpp)
target_link_libraries(dettivo-daemon-client-test PRIVATE dettivo-daemon-client Qt6::Test)
add_test(NAME dettivo-daemon-client-test COMMAND dettivo-daemon-client-test)
set_tests_properties(dettivo-daemon-client-test PROPERTIES
    ENVIRONMENT "QT_QPA_PLATFORM=offscreen")
