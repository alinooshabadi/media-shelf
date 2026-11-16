#ifndef _UNISTD_H
#define _UNISTD_H
#define _SC_NPROCESSORS_ONLN 84
long sysconf(int name);
int usleep(unsigned usec);
#endif
