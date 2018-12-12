#ifndef PAHKATC_H
#define PAHKATC_H

#include <stdlib.h>
#include <stdbool.h>
#include <stdint.h>
#include <sys/types.h>

#ifndef _Nonnull
#define _Nonnull
#endif

typedef void pahkat_client_t;
typedef void pahkat_package_t;
typedef struct pahkat_repo_s {
    const char* url;
    const char* channel;
} pahkat_repo_t;

typedef struct pahkat_error_s {
    uint32_t code;
    const char* message;
} pahkat_error_t;

typedef struct pahkat_action_s {
    const uint8_t action;
    const uint8_t target;
    const char* _Nonnull package_key;
} pahkat_action_t;

typedef struct pahkat_transaction_s pahkat_transaction_t;

enum {
    pahkat_success = 0,
    pahkat_package_download_error,
    pahkat_package_dependency_error,
    pahkat_package_action_contradiction,
    pahkat_package_resolve_error,
    pahkat_package_key_error
};

extern pahkat_client_t*
pahkat_client_new(const char* config_path);

extern const char* _Nonnull
pahkat_config_path(pahkat_client_t* _Nonnull handle);

extern const char*
pahkat_config_ui_get(pahkat_client_t* _Nonnull handle, const char* _Nonnull key);

extern void
pahkat_config_ui_set(pahkat_client_t* _Nonnull handle, const char* _Nonnull key, const char* value);

extern const char* _Nonnull
pahkat_config_repos(pahkat_client_t* _Nonnull handle);

extern void
pahkat_config_set_repos(pahkat_client_t* _Nonnull handle, const char* repos);

extern void
pahkat_refresh_repos(pahkat_client_t* _Nonnull handle);

// extern void
// pahkat_config_add_repo()

extern void
pahkat_client_free(pahkat_client_t* _Nonnull handle);

extern const char* _Nonnull
pahkat_repos_json(const pahkat_client_t* _Nonnull handle);

extern const char*
pahkat_status(
    const pahkat_client_t* _Nonnull handle,
    const char* _Nonnull package_id,
    uint32_t* error);

extern void
pahkat_str_free(const char* _Nonnull str);

extern void
pahkat_error_free(pahkat_error_t* _Nonnull error);

// extern void
// pahkat_add_repo(const pahkat_client_t* _Nonnull handle, const char* repo_url);

// extern void
// pahkat_remove_repo(const pahkat_client_t* _Nonnull handle, const char* repo_url);

// extern uint32_t
// pahkat_list_repos(const pahkat_client_t* _Nonnull handle, pahkat_repo_t** repos);

// extern void
// pahkat_free_repos_list(pahkat_repo_t** repos);

// extern uint32_t
// pahkat_list_packages(const pahkat_client_t* _Nonnull handle, pahkat_repo_t* repo, pahkat_package_t** packages);

// extern void
// pahkat_free_packages_list(pahkat_package_t** packages);

extern pahkat_action_t*
pahkat_create_action(uint8_t action, uint8_t target, const char* _Nonnull package_key);

extern void
pahkat_free_action(pahkat_action_t* _Nonnull action);

extern uint32_t /* error */
pahkat_download_package(
    const pahkat_client_t* _Nonnull handle,
    const char* _Nonnull package_key,
    uint8_t target,
    void (*progress)(const char* /* package_id */, uint64_t /* cur */, uint64_t /* max */),
    pahkat_error_t** error
);

extern pahkat_transaction_t* _Nonnull
pahkat_create_package_transaction(
    const pahkat_client_t* _Nonnull handle,
    const uint32_t action_count,
    const pahkat_action_t* _Nonnull actions,
    pahkat_error_t** error
);

extern uint32_t
pahkat_validate_package_transaction(
    const pahkat_client_t* _Nonnull handle,
    const pahkat_transaction_t* _Nonnull transaction,
    pahkat_error_t** error
);

extern uint32_t
pahkat_run_package_transaction(
    const pahkat_client_t* _Nonnull handle,
    pahkat_transaction_t* _Nonnull transaction,
    uint32_t tx_id,
    void (*progress)(uint32_t, const char* /* package_id */, uint32_t /* action */),
    pahkat_error_t** error
);

extern const char* _Nonnull
pahkat_package_transaction_packages(
    const pahkat_client_t* _Nonnull handle,
    const pahkat_transaction_t _Nonnull transaction,
    pahkat_error_t** error
);

// extern uint32_t /* error */
// pakhat_install_package(const pahkat_client_t* _Nonnull handle,
//     const char* package_key,
//     uint8_t target);

// extern uint32_t /* error */
// pakhat_uninstall_package(const pahkat_client_t* _Nonnull handle,
//     const char* package_key,
//     uint8_t target);

// extern void
// pahkat_package_status(const pahkat_client_t* _Nonnull handle, const char* package_id, uint8_t target, uint32_t* error);

// extern const char*
// pahkat_error(uint32_t error);

// TODO: 

#endif
