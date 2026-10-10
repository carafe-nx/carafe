include_guard(GLOBAL)

set(CARAFE_OVERLAY_FILE "${CMAKE_CURRENT_LIST_DIR}/carafe_overlay.c")
set(CARAFE_LOADING_FILE "${CMAKE_CURRENT_LIST_DIR}/carafe_loading.c")
set(CARAFE_DIAG_FILE "${CMAKE_CURRENT_LIST_DIR}/carafe_diag.c")
set(CARAFE_PATCH_DIR "${CMAKE_CURRENT_LIST_DIR}/../patches/autorun")
set(CARAFE_WRAPPED_SYMBOLS
    wine_nx_osk_visible
    wine_nx_osk_generation
    wine_nx_osk_frame
    wine_nx_osk_copy
    wine_nx_threads_report_stalled
    wine_nx_compositor_cursor
    wine_nx_vk_reserve_window
)

function(carafe_patch_target_sources original copy)
    get_property(targets DIRECTORY "${CMAKE_SOURCE_DIR}" PROPERTY BUILDSYSTEM_TARGETS)
    foreach(target IN LISTS targets)
        get_target_property(sources ${target} SOURCES)
        if(NOT sources)
            continue()
        endif()
        set(replaced FALSE)
        set(patched_sources "")
        foreach(source IN LISTS sources)
            get_filename_component(absolute "${source}" ABSOLUTE BASE_DIR "${CMAKE_SOURCE_DIR}")
            if(absolute STREQUAL original)
                list(APPEND patched_sources "${copy}")
                set(replaced TRUE)
            else()
                list(APPEND patched_sources "${source}")
            endif()
        endforeach()
        if(replaced)
            get_filename_component(original_dir "${original}" DIRECTORY)
            set_property(SOURCE "${copy}" DIRECTORY "${CMAKE_SOURCE_DIR}"
                APPEND PROPERTY COMPILE_OPTIONS "-iquote${original_dir}")
            set_property(TARGET ${target} PROPERTY SOURCES "${patched_sources}")
            message(STATUS "Carafe patch: ${target} builds ${copy}")
        endif()
    endforeach()
endfunction()

function(carafe_apply_patches)
    get_filename_component(autorun_root "${CMAKE_SOURCE_DIR}/.." ABSOLUTE)
    file(GLOB patches CONFIGURE_DEPENDS "${CARAFE_PATCH_DIR}/*.patch")
    list(SORT patches)
    set(copied "")
    foreach(patch IN LISTS patches)
        set_property(DIRECTORY "${CMAKE_SOURCE_DIR}" APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${patch}")
        file(STRINGS "${patch}" targets REGEX "^\\+\\+\\+ b/")
        list(LENGTH targets target_count)
        if(NOT target_count EQUAL 1)
            message(FATAL_ERROR "Carafe patch ${patch}: one file per patch, found ${target_count}")
        endif()
        string(REGEX REPLACE "^\\+\\+\\+ b/([^\t ]+).*$" "\\1" relative "${targets}")
        set(original "${autorun_root}/${relative}")
        set(copy_root "${CMAKE_BINARY_DIR}/carafe-patched")
        set(copy "${copy_root}/${relative}")
        list(FIND copied "${relative}" seen)
        if(seen EQUAL -1)
            get_filename_component(copy_dir "${copy}" DIRECTORY)
            file(MAKE_DIRECTORY "${copy_dir}")
            file(COPY_FILE "${original}" "${copy}")
            list(APPEND copied "${relative}")
        endif()
        execute_process(
            COMMAND patch --batch --forward -p1 -d "${copy_root}" -i "${patch}"
            RESULT_VARIABLE result
            OUTPUT_VARIABLE output
            ERROR_VARIABLE output
        )
        if(NOT result EQUAL 0)
            message(FATAL_ERROR "Carafe patch ${patch} does not apply:\n${output}")
        endif()
        if(seen EQUAL -1)
            set_property(DIRECTORY "${CMAKE_SOURCE_DIR}" APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${original}")
            carafe_patch_target_sources("${original}" "${copy}")
        endif()
    endforeach()
endfunction()

function(carafe_add_overlay)
    if(NOT TARGET wine-nx-runtime)
        message(FATAL_ERROR "Carafe overlay: target wine-nx-runtime not found")
    endif()
    target_sources(
        wine-nx-runtime
        PRIVATE "${CARAFE_OVERLAY_FILE}" "${CARAFE_LOADING_FILE}" "${CARAFE_DIAG_FILE}"
    )
    target_compile_definitions(wine-nx-runtime PRIVATE ${CARAFE_OVERLAY_DEFINITIONS})
    set_source_files_properties(
        "${CARAFE_LOADING_FILE}"
        PROPERTIES
            INCLUDE_DIRECTORIES "${CMAKE_SOURCE_DIR}/source;$ENV{DEVKITPRO}/portlibs/switch/include/SDL2"
            COMPILE_DEFINITIONS _REENTRANT
    )
    foreach(symbol IN LISTS CARAFE_WRAPPED_SYMBOLS)
        target_link_options(wine-nx-runtime PRIVATE "LINKER:--wrap=${symbol}")
    endforeach()
    carafe_apply_patches()
    message(STATUS "Carafe overlay: overlay, loading screen and diagnostics added to wine-nx-runtime")
endfunction()

cmake_language(DEFER DIRECTORY "${CMAKE_SOURCE_DIR}" CALL carafe_add_overlay)
