#include "cpp/network_shim.hpp"

#include "crt/src/network_ffi.rs.h"

#include <cstdint>
#include <map>
#include <string>
#include <utility>

#include "Reader/Reader.hpp"
#include "Config/config.h"

Config *Config::instance = nullptr;

namespace netshim
{

    Network read_network(rust::Str network_file, rust::Str source_dir)
    {
        std::string nf(network_file.data(), network_file.size());
        std::string sd(source_dir.data(), source_dir.size());

        Config *c = c->getInstance();
        c->set_source_dir(sd.c_str());
        c->set_network_file(nf.c_str());

        IR::Dataflow_Network *dpn = Network_Reader::read_network();

        Network net;
        net.name = rust::String(dpn->get_name());

        for (auto &kv : dpn->get_id_class_map())
        {
            Instance inst;
            inst.id = rust::String(kv.first);
            inst.class_name = rust::String(kv.second);

            std::map<std::string, std::string> params;
            dpn->get_params_for_instance(kv.first, params);
            for (auto &p : params)
            {
                inst.parameters.push_back(Param{rust::String(p.first), rust::String(p.second)});
            }
            net.instances.push_back(std::move(inst));
        }

        for (auto &e : dpn->get_edges())
        {
            net.edges.push_back(Edge{
                rust::String(e.get_src_id()),
                rust::String(e.get_src_port()),
                rust::String(e.get_dst_id()),
                rust::String(e.get_dst_port()),
                static_cast<std::uint32_t>(e.get_specified_size()),
            });
        }

        for (auto &kv : dpn->get_actors_class_path_map())
        {
            net.class_paths.push_back(ClassPath{rust::String(kv.first), rust::String(kv.second)});
        }

        delete dpn;
        return net;
    }
}
