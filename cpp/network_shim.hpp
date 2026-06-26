#pragma once

#include "rust/cxx.h"

namespace netshim
{

    struct Network;

    Network read_network(rust::Str network_file, rust::Str source_dir);

}
