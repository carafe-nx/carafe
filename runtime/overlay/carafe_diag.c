void __real_wine_nx_threads_report_stalled(void);
void carafe_diag_dump(void) __attribute__((weak));
void carafe_alert_dump(void) __attribute__((weak));

void __wrap_wine_nx_threads_report_stalled(void) {
    __real_wine_nx_threads_report_stalled();
    if (carafe_diag_dump) {
        carafe_diag_dump();
    }
    if (carafe_alert_dump) {
        carafe_alert_dump();
    }
}
