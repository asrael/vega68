#ifndef V68_BIOS_H
#define V68_BIOS_H

#include "sys.h"

#define V68_CART_RAM 0x02020000
#define V68_RAM_END  0x02400000

#define V68_LEN(a) (sizeof(a) / sizeof((a)[0]))

void v68_fault_dump(void);
void __attribute__((noreturn)) v68_monitor(const char *msg);
void v68_rte_stub(void);

extern u32 v68_fault_regs[17];
extern u32 *const v68_monitor_sp;

#endif
